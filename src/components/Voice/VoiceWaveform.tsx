import { useEffect, useRef } from "react";
import "./VoiceWaveform.css";

const BAR_COUNT = 96;

type VoiceWaveformProps = {
  level: number;
};

export function VoiceWaveform({ level }: VoiceWaveformProps) {
  const containerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const bars = Array.from(container.children) as HTMLElement[];
    const clamped = Number.isFinite(level)
      ? Math.max(0, Math.min(1, level))
      : 0;

    // Boost quiet input so even soft speech moves the bars hard.
    const amplitude = Math.min(1, Math.pow(clamped, 0.35) * 1.3);
    const jitterAmount = 2 * amplitude;

    bars.forEach((bar, index) => {
      // One static sine cycle spans the full width.
      const progress = bars.length > 1 ? index / (bars.length - 1) : 0;
      const wave = (Math.sin(progress * Math.PI * 2) + 1) / 2;
      const shape = Math.pow(wave, 1.5);
      const jitter = 1 + (Math.random() - 0.5) * jitterAmount;
      const scale = Math.max(
        0.04,
        Math.min(1, (0.04 + 0.96 * shape * amplitude) * jitter),
      );
      bar.style.transform = `scaleY(${scale.toFixed(3)})`;
    });
  }, [level]);

  return (
    <div className="voice-waveform" aria-hidden="true" ref={containerRef}>
      {Array.from({ length: BAR_COUNT }, (_, index) => (
        <span key={index} />
      ))}
    </div>
  );
}
