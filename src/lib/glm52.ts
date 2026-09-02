import type { BalanceItem, QuotaInfo } from "./api";

/**
 * 自动切号判定模型的默认值。
 * 与旧版"严格 GLM-5.3 Flash 口径"一致，老用户升级后行为不变。
 */
export const DEFAULT_AUTO_SWITCH_MODEL = "GLM-5.3 Flash";

/** 额度数据还没拉到时，判定模型下拉框兜底展示的常见条目。 */
export const FALLBACK_AUTO_SWITCH_MODELS = [
  "GLM-5.3 Flash",
  "GLM-5.3",
  "GLM-5.2",
];

/**
 * 归一化模型名：小写并去掉所有分隔符（空格、连字符、点等），只保留字母/数字/汉字。
 * "GLM-5.3 Flash"、"glm-5.3-flash"、"GLM 5.3 FLASH" 视为同一模型；
 * "GLM-5.3" 与 "GLM-5.3 Flash" 归一化后不相等，不会互相误匹配。
 */
export function normalizeModelName(name: string): string {
  return name.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "");
}

/**
 * 在账号额度里找所选判定模型对应的条目——自动切号 / 排序 / 悬浮窗统一用这一个口径。
 *
 * 优先归一化精确匹配；没有时用包含匹配兜底（应对接口在名字上追加后缀，
 * 例如所选 "GLM-5.3 Flash" 匹配 "GLM-5.3 Flash 每日"）。找不到返回 undefined，
 * 表示该账号没有此模型的额度数据，不能据此判断可用性。
 */
export function findModelBalance(
  quota: QuotaInfo | undefined,
  model: string
): BalanceItem | undefined {
  const target = normalizeModelName(model);
  if (!target || !quota?.balances?.length) return undefined;
  return (
    quota.balances.find((b) => normalizeModelName(b.show_name) === target) ??
    quota.balances.find((b) => normalizeModelName(b.show_name).includes(target))
  );
}

/** 所选判定模型的剩余额度；没有该模型条目或数据不完整时返回 null。 */
export function modelRemaining(
  quota: QuotaInfo | undefined,
  model: string
): number | null {
  const item = findModelBalance(quota, model);
  if (!item) return null;
  if (Number.isFinite(item.remaining_units)) return item.remaining_units;
  if (Number.isFinite(item.total_units) && Number.isFinite(item.used_units)) {
    return Math.max(0, item.total_units - item.used_units);
  }
  return null;
}

/**
 * 汇总所有账号额度里出现过的模型名，作为判定模型下拉框的选项。
 * 按归一化名去重（保留先出现的原始写法），一个都没有时回落到常见列表。
 */
export function collectModelNames(quotas: Record<string, QuotaInfo>): string[] {
  const byKey = new Map<string, string>();
  for (const quota of Object.values(quotas)) {
    for (const item of quota?.balances ?? []) {
      const name = item.show_name.trim();
      const key = normalizeModelName(name);
      if (key && !byKey.has(key)) byKey.set(key, name);
    }
  }
  if (byKey.size === 0) return [...FALLBACK_AUTO_SWITCH_MODELS];
  return [...byKey.values()].sort((a, b) =>
    a.localeCompare(b, undefined, { numeric: true })
  );
}
