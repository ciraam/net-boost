use std::net::{IpAddr, Ipv4Addr, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::Local;
use tauri::{AppHandle, Emitter};
use windows::Win32::NetworkManagement::IpHelper::{
    IcmpCloseHandle, IcmpCreateFile, IcmpSendEcho, ICMP_ECHO_REPLY, IP_OPTION_INFORMATION,
    IP_SUCCESS, IP_TTL_EXPIRED_TRANSIT,
};

use crate::logger::CommandLogger;
use crate::security::sanitize_host;
use crate::types::{
    BeforeAfterComparison, PingEventPayload, PingSample, PingStats, TracerouteEvent, TracerouteHop,
};

pub fn resolve_ipv4(host: &str) -> Result<Ipv4Addr, String> {
    let clean = sanitize_host(host)?;
    if let Ok(v4) = clean.parse::<Ipv4Addr>() {
        return Ok(v4);
    }
    let addrs = format!("{}:0", clean)
        .to_socket_addrs()
        .map_err(|e| format!("Résolution DNS impossible pour {} : {}", clean, e))?;
    for a in addrs {
        if let IpAddr::V4(v4) = a.ip() {
            return Ok(v4);
        }
    }
    Err(format!("Aucune adresse IPv4 résolue pour {}", clean))
}

#[derive(Debug, Clone)]
pub struct IcmpReplyInfo {
    pub rtt_ms: f64,
    pub responder: Ipv4Addr,
    pub status: u32,
}

/// Native Win32 ICMP Echo via iphlpapi::IcmpSendEcho (zero ping.exe / tracert.exe child process).
pub fn icmp_send_echo_v4(dest: Ipv4Addr, timeout_ms: u32, ttl: Option<u8>) -> Option<IcmpReplyInfo> {
    unsafe {
        let handle = IcmpCreateFile().ok()?;
        let payload = b"NetBoostTelemetryICMP0123456789!";
        let mut reply_buf = vec![0u8; std::mem::size_of::<ICMP_ECHO_REPLY>() + payload.len() + 64];
        let dest_u32 = u32::from_ne_bytes(dest.octets());

        let mut opts = IP_OPTION_INFORMATION::default();
        let opt_ptr = if let Some(t) = ttl {
            opts.Ttl = t;
            Some(&opts as *const IP_OPTION_INFORMATION)
        } else {
            None
        };

        let count = IcmpSendEcho(
            handle,
            dest_u32,
            payload.as_ptr() as *const _,
            payload.len() as u16,
            opt_ptr,
            reply_buf.as_mut_ptr() as *mut _,
            reply_buf.len() as u32,
            timeout_ms,
        );

        let _ = IcmpCloseHandle(handle);

        if count == 0 {
            return None;
        }

        let reply = &*(reply_buf.as_ptr() as *const ICMP_ECHO_REPLY);
        let responder = Ipv4Addr::from(reply.Address.to_ne_bytes());
        let rtt_ms = if reply.RoundTripTime == 0 {
            0.5
        } else {
            reply.RoundTripTime as f64
        };

        if reply.Status == IP_SUCCESS || reply.Status == IP_TTL_EXPIRED_TRANSIT {
            Some(IcmpReplyInfo {
                rtt_ms,
                responder,
                status: reply.Status,
            })
        } else {
            None
        }
    }
}

pub fn ping_once_native(host: &str, timeout_ms: u32) -> Result<Option<f64>, String> {
    let dest = resolve_ipv4(host)?;
    Ok(icmp_send_echo_v4(dest, timeout_ms, None)
        .filter(|r| r.status == IP_SUCCESS)
        .map(|r| r.rtt_ms))
}

pub fn compute_ping_stats(samples: &[PingSample]) -> PingStats {
    if samples.is_empty() {
        return PingStats::default();
    }
    let sent = samples.len() as u64;
    let rtts: Vec<f64> = samples.iter().filter_map(|s| s.rtt_ms).collect();
    let received = rtts.len() as u64;
    let lost = sent - received;
    let loss_pct = ((lost as f64 / sent as f64) * 1000.0).round() / 10.0;

    if rtts.is_empty() {
        return PingStats {
            sent,
            received: 0,
            lost,
            loss_pct,
            current_ms: None,
            avg_ms: None,
            min_ms: None,
            max_ms: None,
            jitter_ms: 0.0,
        };
    }

    let sum: f64 = rtts.iter().sum();
    let avg_ms = ((sum / rtts.len() as f64) * 10.0).round() / 10.0;
    let min_ms = rtts.iter().cloned().fold(f64::INFINITY, f64::min);
    let max_ms = rtts.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    let current_ms = *rtts.last().unwrap();

    let mut jitter_sum = 0.0;
    let mut jitter_pairs = 0usize;
    for w in rtts.windows(2) {
        jitter_sum += (w[1] - w[0]).abs();
        jitter_pairs += 1;
    }
    let jitter_ms = if jitter_pairs > 0 {
        ((jitter_sum / jitter_pairs as f64) * 10.0).round() / 10.0
    } else {
        0.0
    };

    PingStats {
        sent,
        received,
        lost,
        loss_pct,
        current_ms: Some(current_ms),
        avg_ms: Some(avg_ms),
        min_ms: Some(min_ms),
        max_ms: Some(max_ms),
        jitter_ms,
    }
}

struct PingInner {
    host: String,
    resolved_ip: Option<Ipv4Addr>,
    phase: String,
    seq: u64,
    all_samples: Vec<PingSample>,
    before_samples: Vec<PingSample>,
    after_samples: Vec<PingSample>,
    cancel_flag: Option<Arc<AtomicBool>>,
}

#[derive(Clone)]
pub struct PingMonitor {
    inner: Arc<Mutex<PingInner>>,
    app: Arc<Mutex<Option<AppHandle>>>,
    logger: CommandLogger,
}

impl PingMonitor {
    pub fn new(logger: CommandLogger) -> Self {
        Self {
            inner: Arc::new(Mutex::new(PingInner {
                host: "1.1.1.1".to_string(),
                resolved_ip: Some(Ipv4Addr::new(1, 1, 1, 1)),
                phase: "before".to_string(),
                seq: 0,
                all_samples: Vec::new(),
                before_samples: Vec::new(),
                after_samples: Vec::new(),
                cancel_flag: None,
            })),
            app: Arc::new(Mutex::new(None)),
            logger,
        }
    }

    pub fn attach_app(&self, handle: AppHandle) {
        if let Ok(mut g) = self.app.lock() {
            *g = Some(handle);
        }
    }

    pub fn set_phase(&self, phase: &str) {
        if let Ok(mut g) = self.inner.lock() {
            g.phase = phase.to_string();
            if phase == "after" {
                g.after_samples.clear();
            }
        }
    }

    pub fn reset_comparison(&self) {
        if let Ok(mut g) = self.inner.lock() {
            g.all_samples.clear();
            g.before_samples.clear();
            g.after_samples.clear();
            g.seq = 0;
        }
    }

    pub fn stop(&self) {
        if let Ok(mut g) = self.inner.lock() {
            if let Some(flag) = g.cancel_flag.take() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    pub fn start(&self, raw_host: &str) -> Result<(), String> {
        let clean = sanitize_host(raw_host)?;
        let dest = resolve_ipv4(&clean)?;

        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut g = self.inner.lock().map_err(|_| "Verrou PingMonitor indisponible".to_string())?;
            if let Some(prev) = g.cancel_flag.take() {
                prev.store(true, Ordering::SeqCst);
            }
            if g.host != clean {
                g.all_samples.clear();
                g.before_samples.clear();
                g.after_samples.clear();
                g.seq = 0;
            }
            g.host = clean.clone();
            g.resolved_ip = Some(dest);
            g.cancel_flag = Some(cancel.clone());
        }

        self.logger.info(
            "monitor",
            &format!("Win32 IcmpSendEcho -> {} ({})", clean, dest),
            &format!("Monitoring ICMP natif (0% spawn) démarré vers {}", clean),
            "ok",
        );

        let inner = self.inner.clone();
        let app_arc = self.app.clone();

        tauri::async_runtime::spawn(async move {
            loop {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                let rtt = tokio::task::spawn_blocking(move || {
                    icmp_send_echo_v4(dest, 1500, None)
                        .filter(|r| r.status == IP_SUCCESS)
                        .map(|r| r.rtt_ms)
                })
                .await
                .ok()
                .flatten();

                if cancel.load(Ordering::SeqCst) {
                    break;
                }

                let payload_opt = {
                    if let Ok(mut g) = inner.lock() {
                        if g.host != clean {
                            None
                        } else {
                            g.seq += 1;
                            let now_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .unwrap_or_default()
                                .as_millis() as i64;
                            let time_label = Local::now().format("%H:%M:%S").to_string();

                            let prev_rtt = g.all_samples.iter().rev().find_map(|s| s.rtt_ms);
                            let instant_jitter = match (rtt, prev_rtt) {
                                (Some(cur), Some(prev)) => ((cur - prev).abs() * 10.0).round() / 10.0,
                                _ => 0.0,
                            };

                            let mut sample = PingSample {
                                seq: g.seq,
                                timestamp: now_ms,
                                time_label,
                                host: clean.clone(),
                                rtt_ms: rtt,
                                jitter_ms: instant_jitter,
                                loss_pct: 0.0,
                                lost: rtt.is_none(),
                                phase: g.phase.clone(),
                            };

                            g.all_samples.push(sample.clone());
                            if g.all_samples.len() > 90 {
                                g.all_samples.remove(0);
                            }

                            if g.phase == "before" {
                                g.before_samples.push(sample.clone());
                                if g.before_samples.len() > 180 {
                                    g.before_samples.remove(0);
                                }
                            } else {
                                g.after_samples.push(sample.clone());
                                if g.after_samples.len() > 180 {
                                    g.after_samples.remove(0);
                                }
                            }

                            let stats = compute_ping_stats(&g.all_samples);
                            sample.loss_pct = stats.loss_pct;
                            let comparison = BeforeAfterComparison {
                                before: compute_ping_stats(&g.before_samples),
                                after: compute_ping_stats(&g.after_samples),
                            };
                            Some(PingEventPayload {
                                sample,
                                stats,
                                comparison,
                            })
                        }
                    } else {
                        None
                    }
                };

                if let Some(payload) = payload_opt {
                    if let Ok(guard) = app_arc.lock() {
                        if let Some(app) = guard.as_ref() {
                            let _ = app.emit("event:ping", &payload);
                        }
                    }
                }

                tokio::time::sleep(Duration::from_millis(1000)).await;
            }
        });

        Ok(())
    }
}

#[derive(Clone)]
pub struct TracerouteRunner {
    cancel_flag: Arc<Mutex<Option<Arc<AtomicBool>>>>,
    app: Arc<Mutex<Option<AppHandle>>>,
    logger: CommandLogger,
}

impl TracerouteRunner {
    pub fn new(logger: CommandLogger) -> Self {
        Self {
            cancel_flag: Arc::new(Mutex::new(None)),
            app: Arc::new(Mutex::new(None)),
            logger,
        }
    }

    pub fn attach_app(&self, handle: AppHandle) {
        if let Ok(mut g) = self.app.lock() {
            *g = Some(handle);
        }
    }

    pub fn cancel(&self) {
        if let Ok(mut g) = self.cancel_flag.lock() {
            if let Some(flag) = g.take() {
                flag.store(true, Ordering::SeqCst);
            }
        }
    }

    fn emit(&self, ev: TracerouteEvent) {
        if let Ok(g) = self.app.lock() {
            if let Some(app) = g.as_ref() {
                let _ = app.emit("event:traceroute", &ev);
            }
        }
    }

    pub fn run(&self, raw_host: &str) -> Result<(), String> {
        let host = sanitize_host(raw_host)?;
        let dest = resolve_ipv4(&host)?;
        self.cancel();

        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut g) = self.cancel_flag.lock() {
            *g = Some(cancel.clone());
        }

        self.logger.info(
            "monitor",
            &format!("Win32 IcmpSendEcho Traceroute (TTL 1..20) -> {} ({})", host, dest),
            &format!("Traceroute ICMP natif lancé vers {}...", host),
            "ok",
        );

        let runner = self.clone();
        tauri::async_runtime::spawn(async move {
            runner.emit(TracerouteEvent {
                event_type: "start".to_string(),
                host: host.clone(),
                hop: None,
                message: None,
            });

            for ttl in 1u8..=20u8 {
                if cancel.load(Ordering::SeqCst) {
                    break;
                }

                let hop_res = tokio::task::spawn_blocking(move || {
                    let mut rtts: [Option<f64>; 3] = [None, None, None];
                    let mut responder: Option<Ipv4Addr> = None;
                    let mut reached_dest = false;

                    for slot in &mut rtts {
                        if let Some(rep) = icmp_send_echo_v4(dest, 1000, Some(ttl)) {
                            *slot = Some(rep.rtt_ms);
                            responder = Some(rep.responder);
                            if rep.status == IP_SUCCESS || rep.responder == dest {
                                reached_dest = true;
                            }
                        }
                    }

                    let valid: Vec<f64> = rtts.iter().filter_map(|v| *v).collect();
                    let avg_ms = if valid.is_empty() {
                        None
                    } else {
                        Some(((valid.iter().sum::<f64>() / valid.len() as f64) * 10.0).round() / 10.0)
                    };

                    let hop = TracerouteHop {
                        hop: ttl as u32,
                        rtt1: rtts[0],
                        rtt2: rtts[1],
                        rtt3: rtts[2],
                        avg_ms,
                        ip: responder.map(|ip| ip.to_string()),
                        timed_out: valid.is_empty() && responder.is_none(),
                    };
                    (hop, reached_dest)
                })
                .await;

                if cancel.load(Ordering::SeqCst) {
                    break;
                }

                if let Ok((hop, reached)) = hop_res {
                    runner.emit(TracerouteEvent {
                        event_type: "hop".to_string(),
                        host: host.clone(),
                        hop: Some(hop),
                        message: None,
                    });
                    if reached {
                        break;
                    }
                }
            }

            runner.logger.info(
                "monitor",
                &format!("Traceroute natif {}", host),
                "Traceroute ICMP terminé.",
                "ok",
            );
            runner.emit(TracerouteEvent {
                event_type: "done".to_string(),
                host,
                hop: None,
                message: None,
            });
        });

        Ok(())
    }
}
