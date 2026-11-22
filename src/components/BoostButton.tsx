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
  const accentColor = active ? '#10B981' : '#5E6AD2';

  return (
    <div className="flex flex-col items-center justify-center py-1">
      <div className="relative flex items-center justify-center w-48 h-48">
        {/* Expanding Pulse Ring */}
        <div
          className={`absolute w-44 h-44 rounded-full border pointer-events-none ${
            active ? 'border-emerald-500/40 animate-pulse-ring' : 'border-primary/30 animate-pulse-ring'
          }`}
        />

        {/* Graduated Telemetry Tick Ring (SVG) */}
        <svg
          viewBox="0 0 200 200"
          className="absolute inset-0 w-full h-full pointer-events-none animate-spin-slow opacity-65"
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
                strokeWidth={major ? '1.6' : '1'}
                strokeOpacity={major ? '0.8' : '0.3'}
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
            strokeWidth="1.25"
            strokeDasharray="48 24 12 24"
            strokeOpacity={active ? '0.65' : '0.4'}
          />
        </svg>

        {/* Core Reactor Button */}
        <button
          onClick={onToggle}
          disabled={busy}
          className={`group relative z-10 w-32 h-32 rounded-full flex flex-col items-center justify-center gap-1 transition-all duration-200 cursor-pointer border active:scale-[0.97] disabled:cursor-wait ${
            active
              ? 'bg-emerald-500/15 hover:bg-emerald-500/20 border-emerald-500/60 text-emerald-600 dark:text-emerald-400 shadow-[0_0_28px_rgba(16,185,129,0.22),inset_0_1px_0_rgba(255,255,255,0.2)] hover:scale-[1.03]'
              : 'bg-primary/15 hover:bg-primary/22 border-primary/60 text-primary shadow-[0_0_28px_rgba(94,106,210,0.26),inset_0_1px_0_rgba(255,255,255,0.2)] hover:scale-[1.03]'
          }`}
        >
          {busy ? (
            <Loader2 className="w-7 h-7 animate-spin" />
          ) : active ? (
            <Power className="w-7 h-7 transition-transform duration-150 group-hover:scale-105" />
          ) : (
            <Zap className="w-7 h-7 transition-transform duration-150 group-hover:scale-105" />
          )}

          <span className="text-base font-semibold tracking-[0.14em] leading-none mt-0.5">
            {busy ? '...' : active ? 'STOP' : 'BOOST'}
          </span>
          <span className="text-[10px] font-mono font-medium uppercase tracking-[0.16em] opacity-80">
            {busy ? 'EXECUTION' : active ? 'LOCKED' : 'ENGAGE'}
          </span>
        </button>
      </div>

      <div className="mt-1.5 text-center space-y-0.5">
        <div
          className="text-[11px] metric-mono font-medium text-muted-foreground truncate max-w-[280px]"
          title={targetLabel}
        >
          {targetLabel}
        </div>
      </div>
    </div>
  );
};
