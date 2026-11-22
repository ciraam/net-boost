import React from 'react';
import { Loader2, Power, Zap } from 'lucide-react';

interface BoostButtonProps {
  active: boolean;
  busy: boolean;
  onToggle: () => void;
  targetLabel: string;
}

const TICK_ANGLES = Array.from({ length: 36 }, (_, i) => i * 10);

export const BoostButton: React.FC<BoostButtonProps> = ({ active, busy, onToggle, targetLabel }) => {
  const accentColor = active ? '#10F5A0' : '#00E5FF';

  return (
    <div className="flex flex-col items-center justify-center py-1">
      <div className="relative flex items-center justify-center w-48 h-48">
        {/* Expanding Sonar Pulse Ring */}
        <div
          className={`absolute w-44 h-44 rounded-full border pointer-events-none ${
            active ? 'border-neon-green/50 animate-pulse-ring' : 'border-neon-cyan/35 animate-pulse-ring'
          }`}
        />

        {/* Graduated Tactical Telemetry Tick Ring (SVG) */}
        <svg
          viewBox="0 0 200 200"
          className="absolute inset-0 w-full h-full pointer-events-none animate-spin-slow opacity-70"
        >
          {TICK_ANGLES.map((deg, idx) => {
            const major = idx % 3 === 0;
            return (
              <line
                key={deg}
                x1="100"
                y1={major ? '6' : '10'}
                x2="100"
                y2={major ? '16' : '15'}
                stroke={accentColor}
                strokeWidth={major ? '1.8' : '1'}
                strokeOpacity={major ? '0.85' : '0.35'}
                transform={`rotate(${deg} 100 100)`}
              />
            );
          })}
        </svg>

        {/* Counter-Rotating Inner Orbital Arc */}
        <svg
          viewBox="0 0 200 200"
          className="absolute w-40 h-40 pointer-events-none animate-spin-reverse"
        >
          <circle
            cx="100"
            cy="100"
            r="92"
            fill="none"
            stroke={accentColor}
            strokeWidth="1.5"
            strokeDasharray="48 24 12 24"
            strokeOpacity={active ? '0.75' : '0.45'}
          />
        </svg>

        {/* Core Reactor Button */}
        <button
          onClick={onToggle}
          disabled={busy}
          className={`group relative z-10 w-32 h-32 rounded-full flex flex-col items-center justify-center gap-1 transition-all duration-300 cursor-pointer border-2 active:scale-[0.96] disabled:cursor-wait ${
            active
              ? 'bg-gradient-to-b from-neon-green/25 via-[#0E1A18] to-[#070A0F] border-neon-green text-neon-green shadow-[0_0_42px_rgba(16,245,160,0.34),inset_0_0_24px_rgba(16,245,160,0.2)] hover:scale-[1.04] hover:shadow-[0_0_54px_rgba(16,245,160,0.5)]'
              : 'bg-gradient-to-b from-neon-cyan/22 via-[#0C1824] to-[#070A0F] border-neon-cyan text-neon-cyan shadow-[0_0_38px_rgba(0,229,255,0.28),inset_0_0_24px_rgba(0,229,255,0.16)] hover:scale-[1.04] hover:shadow-[0_0_54px_rgba(0,229,255,0.48)]'
          }`}
        >
          {busy ? (
            <Loader2 className="w-8 h-8 animate-spin" />
          ) : active ? (
            <Power className="w-8 h-8 transition-transform duration-300 group-hover:scale-110 drop-shadow-[0_0_12px_rgba(16,245,160,0.9)]" />
          ) : (
            <Zap className="w-8 h-8 transition-transform duration-300 group-hover:scale-110 drop-shadow-[0_0_12px_rgba(0,229,255,0.9)]" />
          )}

          <span className="text-lg font-black tracking-[0.2em] leading-none mt-0.5">
            {busy ? '...' : active ? 'STOP' : 'BOOST'}
          </span>
          <span className="text-[9px] font-mono font-bold uppercase tracking-[0.22em] opacity-85">
            {busy ? 'EXECUTION' : active ? 'LOCKED' : 'ENGAGE'}
          </span>
        </button>
      </div>

      <div className="mt-1.5 text-center space-y-0.5">
        <div
          className="text-[11px] metric-mono font-semibold text-text-secondary truncate max-w-[280px]"
          title={targetLabel}
        >
          {targetLabel}
        </div>
      </div>
    </div>
  );
};
