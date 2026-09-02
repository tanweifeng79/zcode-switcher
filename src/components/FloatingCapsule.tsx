import { findModelBalance } from "../lib/glm52";
import type { ProfileView, QuotaInfo } from "../lib/api";

/** 悬浮胶囊窗口基准尺寸（App 按 scale 缩放窗口大小）。 */
export const FLOATING_BASE_W = 340;
export const FLOATING_BASE_H = 200;
/** 缩放调节面板展开时额外增加的高度（不参与 scale）。 */
export const FLOATING_RESIZER_EXTRA_H = 44;

const TEXTS = {
  zh: {
    pool: "账号池",
    remaining: "剩余",
    noQuota: "暂无额度",
    low: "低额度",
    size: "大小",
  },
  en: {
    pool: "Account Pool",
    remaining: "left",
    noQuota: "No quota",
    low: "Low",
    size: "Size",
  },
} as const;

type Lang = keyof typeof TEXTS;

function fmtUnits(n: number): string {
  if (n >= 100000000) return (n / 100000000).toFixed(2) + "亿";
  if (n >= 10000) return Math.round(n / 10000) + "万";
  return String(n);
}

interface FloatingCapsuleProps {
  profiles: ProfileView[];
  quotas: Record<string, QuotaInfo>;
  thresholdWan: number;
  /** 自动切号判定模型，额度速览与低额度标记跟随同一口径 */
  autoSwitchModel: string;
  language: string;
  scale: number;
  resizerOpen: boolean;
  onScaleChange: (v: number) => void;
  onToggleResizer: () => void;
  onClose: () => void;
}

/** 简化版悬浮胶囊：账号池按判定模型的剩余额度速览 + 缩放调节 + 关闭。 */
export default function FloatingCapsule({
  profiles,
  quotas,
  thresholdWan,
  autoSwitchModel,
  language,
  scale,
  resizerOpen,
  onScaleChange,
  onToggleResizer,
  onClose,
}: FloatingCapsuleProps) {
  const texts = TEXTS[(language as Lang) in TEXTS ? (language as Lang) : "zh"];

  return (
    <div
      className="flex h-full w-full flex-col overflow-hidden rounded-xl border border-white/10 bg-base-100/95 shadow-2xl"
      style={{ transform: `scale(${scale})`, transformOrigin: "top left" }}
    >
      <div className="flex items-center justify-between px-3 pt-2">
        <span className="text-xs font-semibold text-text-primary">
          {texts.pool}（{profiles.length}）
        </span>
        <div className="flex items-center gap-1">
          <button
            className="btn btn-ghost btn-xs px-1 text-text-muted"
            title={texts.size}
            onClick={onToggleResizer}
          >
            ⤢
          </button>
          <button
            className="btn btn-ghost btn-xs px-1 text-text-muted"
            onClick={onClose}
          >
            ✕
          </button>
        </div>
      </div>

      <div className="flex-1 space-y-1 overflow-auto px-3 py-1">
        {profiles.map((p) => {
          const quota = quotas[p.id];
          const glm = findModelBalance(quota, autoSwitchModel);
          const low =
            !!glm && glm.remaining_units < thresholdWan * 10000;
          return (
            <div
              key={p.id}
              className={`flex items-center justify-between rounded-lg px-2 py-1 text-xs ${
                p.active ? "bg-primary/10" : ""
              }`}
            >
              <span className="truncate text-text-primary">
                {p.active ? "● " : ""}
                {p.name}
              </span>
              <span className={low ? "text-warning" : "text-text-muted"}>
                {glm
                  ? `${texts.remaining} ${fmtUnits(glm.remaining_units)}`
                  : texts.noQuota}
              </span>
            </div>
          );
        })}
      </div>

      {resizerOpen && (
        <div className="flex items-center gap-2 border-t border-white/10 px-3 py-2">
          <span className="shrink-0 text-xs text-text-muted">{texts.size}</span>
          <input
            type="range"
            min={0.6}
            max={2}
            step={0.05}
            value={scale}
            onChange={(e) => onScaleChange(Number(e.target.value))}
            className="range range-xs flex-1"
          />
        </div>
      )}
    </div>
  );
}
