import type { AssetPreference } from "@/lib/model";

export function AssetPreferenceFields({ value, onChange, disabled = false }: {
  value: AssetPreference;
  onChange(value: AssetPreference): void;
  disabled?: boolean;
}) {
  return <div className="asset-preferences">
    <label>目标平台<select aria-label="目标平台" disabled={disabled} value={value.platform}
      onChange={event => onChange({ ...value, platform: event.target.value as AssetPreference["platform"] })}>
      <option value="windows">Windows</option><option value="macos">macOS</option>
      <option value="linux">Linux</option><option value="any">不限平台</option>
    </select></label>
    <label>架构<select aria-label="目标架构" disabled={disabled} value={value.architecture}
      onChange={event => onChange({ ...value, architecture: event.target.value as AssetPreference["architecture"] })}>
      <option value="x64">x64</option><option value="arm64">ARM64</option>
      <option value="x86">x86</option><option value="any">不限架构</option>
    </select></label>
    <label>分发类型<select aria-label="分发类型" disabled={disabled} value={value.package}
      onChange={event => onChange({ ...value, package: event.target.value as AssetPreference["package"] })}>
      <option value="installer">安装版</option><option value="portable">便携版 / 压缩包</option>
      <option value="any">不限类型</option>
    </select></label>
  </div>;
}
