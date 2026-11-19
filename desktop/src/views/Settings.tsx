import appIcon from "@/assets/app-icon.svg";
import { PageHeader, usePageActiveRef } from "@/components/PageActivity";
import {
  createContext,
  useContext,
  useEffect,
  useId,
  useRef,
  useState,
  type KeyboardEvent,
  type ReactNode,
} from "react";
import {
  BellRing,
  Bot,
  Check,
  Copy,
  Database,
  Download,
  ExternalLink,
  FolderOpen,
  FolderPlus,
  Globe,
  Info,
  RotateCcw,
  Search,
  SlidersHorizontal,
  X,
  type LucideIcon,
} from "lucide-react";
import { notify, request, useWorkspace } from "@/lib/api";
import {
  limitMegabytes,
  parseLimit,
  settingsFingerprint,
  type Settings as SettingsModel,
  type UpdateResult,
} from "@/lib/model";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Modal } from "@/components/ui/dialog";
import { useConfirm, useJob } from "@/components/common";
import { Markdown, openRepository } from "./NewDownload";
import { RouteCheck } from "./Routes";
import { setZoom, useZoom, ZOOM_LEVELS } from "@/lib/zoom";
import {
  ACCENTS,
  setAccent,
  setThemeMode,
  useAppearance,
  type ThemeMode,
} from "@/lib/appearance";
import notices from "../../THIRD_PARTY_NOTICES.md?raw";

// 改编自 magpie 的 set-tabs、prefs、segs 与设置搜索（MIT，固定来源见第三方声明）。
const settingsTabs = [
  ["general", "通用", SlidersHorizontal],
  ["downloads", "下载", Download],
  ["network", "网络", Globe],
  ["updates", "关注更新", BellRing],
  ["agent", "Agent 接入", Bot],
  ["data", "数据与备份", Database],
  ["about", "关于", Info],
] as const satisfies readonly (readonly [string, string, LucideIcon])[];
type SettingsTab = (typeof settingsTabs)[number][0];
const SettingsQuery = createContext("");

function Preference({
  title,
  description,
  htmlFor,
  children,
  wide = false,
  keywords = "",
}: {
  title: string;
  description?: string;
  htmlFor?: string;
  children: ReactNode;
  wide?: boolean;
  keywords?: string;
}) {
  const query = useContext(SettingsQuery);
  const words = query.trim().toLocaleLowerCase().split(/\s+/);
  const text = `${title} ${description ?? ""} ${keywords}`.toLocaleLowerCase();
  if (!words.every((word) => text.includes(word))) return null;
  return (
    <div className={`settings-row${wide ? " settings-row-wide" : ""}`}>
      <div className="settings-row-copy">
        {htmlFor ? (
          <label htmlFor={htmlFor}>{title}</label>
        ) : (
          <strong>{title}</strong>
        )}
        {description && <p>{description}</p>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );
}

function SettingToggle({
  title,
  description,
  checked,
  onChange,
}: {
  title: string;
  description?: string;
  checked: boolean;
  onChange(value: boolean): void;
}) {
  const id = useId();
  return (
    <Preference title={title} description={description}>
      <fieldset className="settings-segment" data-on={checked}>
        <legend className="sr-only">{title}</legend>
        <span className="settings-segment-thumb" aria-hidden="true" />
        {[false, true].map((value) => (
          <label key={String(value)}>
            <input
              type="radio"
              name={id}
              checked={checked === value}
              onChange={() => onChange(value)}
            />
            <span>{value ? "开" : "关"}</span>
          </label>
        ))}
      </fieldset>
    </Preference>
  );
}

function SettingsPanel({
  id,
  title,
  hidden,
  children,
}: {
  id: SettingsTab;
  title: string;
  hidden: boolean;
  children: ReactNode;
}) {
  const searching = !!useContext(SettingsQuery).trim();
  return (
    <section
      className="settings-tabpanel"
      id={`settings-panel-${id}`}
      role="tabpanel"
      aria-labelledby={searching ? undefined : `settings-tab-${id}`}
      aria-label={searching ? title : undefined}
      hidden={hidden}
    >
      <h2 className="settings-section-title">{title}</h2>
      {children}
    </section>
  );
}

/** 分类内的一组设置；搜索时整组都没有匹配项会自动隐藏。 */
function SettingsGroup({
  title,
  children,
}: {
  title: string;
  children: ReactNode;
}) {
  return (
    <div className="settings-group">
      <h3>{title}</h3>
      <div className="settings-preferences">{children}</div>
    </div>
  );
}

/** 从资源管理器选择文件夹的列表，不需要手动输入路径。 */
function DirectoryList({
  value,
  onChange,
  disabled,
}: {
  value: string[];
  onChange(value: string[]): void;
  disabled: boolean;
}) {
  const { busy, run, active } = useJob();
  const add = () =>
    run(async () => {
      const path = await request<string | null>("chooseDirectory", {
        initial: value[value.length - 1] ?? null,
      });
      if (!path || !active.current) return;
      if (value.some((item) => item.toLowerCase() === path.toLowerCase())) {
        notify("这个文件夹已经在列表里了");
        return;
      }
      onChange([...value, path]);
    });
  return (
    <div className="directory-list">
      {value.length ? (
        <ul>
          {value.map((path) => (
            <li key={path}>
              <FolderOpen size={14} aria-hidden="true" />
              <span title={path}>{path}</span>
              <Button
                size="icon"
                variant="ghost"
                aria-label={`移除 ${path}`}
                disabled={disabled || busy}
                onClick={() => onChange(value.filter((item) => item !== path))}
              >
                <X size={14} />
              </Button>
            </li>
          ))}
        </ul>
      ) : (
        <p className="muted">还没有添加文件夹。</p>
      )}
      <Button size="sm" disabled={disabled || busy} onClick={add}>
        <FolderPlus size={14} />
        添加文件夹
      </Button>
    </div>
  );
}

/** 自定义镜像列表：输入地址后添加，每项可以移除。 */
function MirrorList({
  value,
  onChange,
  disabled,
}: {
  value: string[];
  onChange(value: string[]): void;
  disabled: boolean;
}) {
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  const add = () => {
    let url: URL;
    try {
      url = new URL(text.trim());
    } catch {
      setError("请输入完整地址，例如 https://example.com/");
      return;
    }
    if (
      url.protocol !== "https:" ||
      url.username ||
      url.password ||
      url.search ||
      url.hash
    ) {
      setError("只支持 https:// 开头、不含账号密码和参数的地址");
      return;
    }
    const normalized = url.href.endsWith("/") ? url.href : `${url.href}/`;
    if (value.includes(normalized)) {
      setError("这个镜像已经在列表里了");
      return;
    }
    if (value.length >= 8) {
      setError("最多添加 8 个镜像");
      return;
    }
    onChange([...value, normalized]);
    setText("");
    setError("");
  };
  return (
    <div className="directory-list">
      {value.length > 0 && (
        <ul>
          {value.map((mirror) => (
            <li key={mirror}>
              <span title={mirror}>{mirror}</span>
              <Button
                size="icon"
                variant="ghost"
                aria-label={`移除 ${mirror}`}
                disabled={disabled}
                onClick={() =>
                  onChange(value.filter((item) => item !== mirror))
                }
              >
                <X size={14} />
              </Button>
            </li>
          ))}
        </ul>
      )}
      <div className="input-row">
        <Input
          aria-label="镜像地址"
          placeholder="https://example.com/"
          value={text}
          disabled={disabled}
          onChange={(event) => {
            setText(event.target.value);
            setError("");
          }}
          onKeyDown={(event) => {
            if (
              event.key === "Enter" &&
              !event.nativeEvent.isComposing &&
              text.trim()
            )
              add();
          }}
        />
        <Button size="sm" disabled={disabled || !text.trim()} onClick={add}>
          添加镜像
        </Button>
      </div>
      {error && <p className="settings-state-note">{error}</p>}
    </div>
  );
}

/** 限速输入框里显示的 MB/s，最多 3 位小数（存储单位是 KiB/s，换算会带出很长的小数）。 */
const showLimit = (kib: number) =>
  String(Number(limitMegabytes(kib).toFixed(3)));
let networkDirtyFlag = false;
/** 只有网络设置需要手动保存；其余设置改完即保存。 */
export function hasUnsavedSettings() {
  return networkDirtyFlag;
}
export function Settings() {
  const visible = usePageActiveRef();
  const { info, snapshot, connected } = useWorkspace();
  const [draft, setDraft] = useState<SettingsModel | null>(
    snapshot?.settings ?? null,
  );
  const [limit, setLimit] = useState(
    showLimit(snapshot?.settings.limitKib ?? 0),
  );
  const saved = useRef<SettingsModel | null>(snapshot?.settings ?? null);
  const queue = useRef<Promise<unknown>>(Promise.resolve());
  const pending = useRef(0);
  const [saveState, setSaveState] = useState<"idle" | "saving" | "saved">(
    "idle",
  );
  const [restartNeeded, setRestartNeeded] = useState(false);
  const settingsEpoch = useRef(0);
  const [tab, setTab] = useState<SettingsTab>("general");
  const [query, setQuery] = useState("");
  const searching = !!query.trim();
  const [target, setTarget] = useState("");
  const [keep, setKeep] = useState(true);
  const [update, setUpdate] = useState<UpdateResult | null>(null);
  const [about, setAbout] = useState(false);
  const { busy, run, active } = useJob();
  const confirm = useConfirm();
  const zoom = useZoom();
  const appearance = useAppearance();
  // 推送来的设置（例如另一处修改）同步到界面；没保存的网络改动保留。
  const pushed = settingsFingerprint(snapshot?.settings);
  useEffect(() => {
    if (!snapshot || pending.current) return;
    saved.current = snapshot.settings;
    setDraft((old) =>
      old && networkDirtyFlag
        ? { ...snapshot.settings, network: old.network }
        : snapshot.settings,
    );
    if (document.activeElement?.id !== "speed-limit")
      setLimit(showLimit(snapshot.settings.limitKib));
  }, [pushed]);
  const networkDirty =
    !!draft &&
    !!saved.current &&
    JSON.stringify(draft.network ?? {}) !==
      JSON.stringify(saved.current.network ?? {});
  useEffect(() => {
    networkDirtyFlag = networkDirty;
  }, [networkDirty]);
  /** 按顺序保存，每次都以上一次成功保存的结果为基础；失败时界面恢复原值。 */
  function persist(patch: Partial<SettingsModel>): Promise<boolean> {
    pending.current++;
    setSaveState("saving");
    const work = queue.current.then(async () => {
      const base = saved.current ?? snapshot!.settings;
      const next = { ...base, ...patch };
      try {
        await request("saveSettings", { settings: next });
        saved.current = next;
        if (active.current) setSaveState("saved");
        return true;
      } catch (error) {
        notify(error instanceof Error ? error.message : String(error), "error");
        if (active.current) {
          setSaveState("idle");
          setDraft((old) =>
            old
              ? { ...base, network: patch.network ? base.network : old.network }
              : old,
          );
        }
        return false;
      } finally {
        pending.current--;
      }
    });
    queue.current = work;
    return work;
  }
  function edit<K extends keyof SettingsModel>(
    key: K,
    value: SettingsModel[K],
  ) {
    setDraft((old) => (old ? { ...old, [key]: value } : old));
  }
  function field<K extends keyof SettingsModel>(
    key: K,
    value: SettingsModel[K],
  ) {
    edit(key, value);
    void persist({ [key]: value } as Partial<SettingsModel>);
  }
  function saveLimit() {
    const current = saved.current?.limitKib ?? 0;
    try {
      const value = parseLimit(limit, current);
      if (value !== current) field("limitKib", value);
      setLimit(showLimit(value));
    } catch (error) {
      notify(error instanceof Error ? error.message : String(error), "error");
      setLimit(showLimit(current));
    }
  }
  function savePattern() {
    if (draft && draft.watchAssetPattern !== saved.current?.watchAssetPattern)
      void persist({ watchAssetPattern: draft.watchAssetPattern });
  }
  const saveNetwork = () =>
    run(async () => {
      if (
        draft &&
        (await persist({ network: draft.network })) &&
        active.current
      )
        setRestartNeeded(true);
    });
  function selectTab(next: SettingsTab) {
    if (tab !== next || query) settingsEpoch.current++;
    setTab(next);
    setQuery("");
  }
  function filterSettings(next: string) {
    if (query !== next) settingsEpoch.current++;
    setQuery(next);
  }
  function tabKeys(event: KeyboardEvent<HTMLDivElement>) {
    const keys = [
      "ArrowLeft",
      "ArrowRight",
      "ArrowUp",
      "ArrowDown",
      "Home",
      "End",
    ];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const index = settingsTabs.findIndex(([key]) => key === tab);
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? settingsTabs.length - 1
          : (index +
              (["ArrowRight", "ArrowDown"].includes(event.key) ? 1 : -1) +
              settingsTabs.length) %
            settingsTabs.length;
    const next = settingsTabs[nextIndex][0];
    selectTab(next);
    event.currentTarget
      .querySelector<HTMLButtonElement>(`#settings-tab-${next}`)
      ?.focus();
  }
  return (
    <section className="page settings-page" tabIndex={-1}>
      <PageHeader>
        <h1>设置</h1>
        <div className="header-actions">
          <span className={`autosave-status is-${saveState}`} role="status">
            {saveState === "saving" ? (
              "正在保存…"
            ) : saveState === "saved" ? (
              <>
                <Check size={14} />
                已自动保存
              </>
            ) : (
              "更改会自动保存"
            )}
          </span>
        </div>
      </PageHeader>
      {draft && (
        <div className="settings-layout">
          <aside className="settings-nav">
            <label className="settings-search">
              <Search size={15} aria-hidden="true" />
              <input
                type="search"
                aria-label="搜索设置"
                placeholder="搜索设置"
                value={query}
                onChange={(e) => filterSettings(e.target.value)}
              />
              {query && (
                <button
                  type="button"
                  aria-label="清除设置搜索"
                  onClick={() => filterSettings("")}
                >
                  <X size={14} />
                </button>
              )}
            </label>
            <div
              className="settings-tabs"
              role="tablist"
              aria-label="设置分类"
              aria-orientation="vertical"
              onKeyDown={tabKeys}
            >
              {settingsTabs.map(([key, label, Icon]) => (
                <button
                  key={key}
                  type="button"
                  id={`settings-tab-${key}`}
                  role="tab"
                  aria-selected={!searching && tab === key}
                  aria-controls={`settings-panel-${key}`}
                  tabIndex={tab === key ? 0 : -1}
                  onClick={() => selectTab(key)}
                >
                  <Icon size={17} strokeWidth={1.7} aria-hidden="true" />
                  {label}
                  {key === "network" && networkDirty && (
                    <i className="unsaved-dot" title="网络设置有未保存的改动" />
                  )}
                </button>
              ))}
            </div>
          </aside>
          <SettingsQuery.Provider value={query}>
            <div className="settings-panels">
              <SettingsPanel
                id="agent"
                title="Agent 接入"
                hidden={!searching && tab !== "agent"}
              >
                <SettingsGroup title="权限">
                  <SettingToggle
                    title="允许本机 Agent 使用 GitHubSP"
                    description="让本机的 AI 助手通过插件查询版本、添加下载。开启后需重启应用，关闭立即生效。"
                    checked={draft.agent?.enabled ?? false}
                    onChange={(enabled) =>
                      field("agent", {
                        enabled,
                        allowedDirectories:
                          draft.agent?.allowedDirectories ?? [],
                      })
                    }
                  />
                  <Preference
                    title="允许的下载目录"
                    description="AI 助手只能把文件下载到这些文件夹及其子文件夹，它不能运行下载的文件。"
                    keywords="目录 文件夹 Agent"
                    wide
                  >
                    <DirectoryList
                      value={(draft.agent?.allowedDirectories ?? []).filter(
                        (value) => value.trim(),
                      )}
                      disabled={busy}
                      onChange={(allowedDirectories) =>
                        field("agent", {
                          enabled: draft.agent?.enabled ?? false,
                          allowedDirectories,
                        })
                      }
                    />
                  </Preference>
                </SettingsGroup>
              </SettingsPanel>
              <SettingsPanel
                id="general"
                title="通用"
                hidden={!searching && tab !== "general"}
              >
                <SettingsGroup title="窗口与通知">
                  <SettingToggle
                    checked={draft.closeToTray}
                    onChange={(v) => field("closeToTray", v)}
                    title="关闭窗口后保留在托盘"
                    description="下载和自动检查会在应用运行期间继续。"
                  />
                  <SettingToggle
                    checked={draft.backgroundCompletionNotice}
                    onChange={(v) => field("backgroundCompletionNotice", v)}
                    title="后台下载完成提醒"
                    description="窗口未处于前台时，显示下载完成提醒。"
                  />
                  <SettingToggle
                    checked={draft.clipboardDetect ?? true}
                    onChange={(v) => field("clipboardDetect", v)}
                    title="识别剪贴板中的 GitHub 链接"
                    description="打开或切回窗口时，若剪贴板里有仓库、版本页或附件链接，在顶部提示下载。只在本机识别，不保存剪贴板内容。"
                  />
                </SettingsGroup>
                <SettingsGroup title="显示">
                  <Preference
                    title="外观"
                    description="浅色、深色，或者跟随 Windows 的设置自动切换。"
                    htmlFor="theme-mode"
                    keywords="主题 深色 暗色 夜间 浅色 外观 跟随系统"
                  >
                    <select
                      id="theme-mode"
                      value={appearance.mode}
                      onChange={(event) => setThemeMode(event.target.value as ThemeMode)}
                    >
                      <option value="light">浅色</option>
                      <option value="dark">深色</option>
                      <option value="system">跟随系统</option>
                    </select>
                  </Preference>
                  <Preference
                    title="主题色"
                    description="按钮、选中项、进度条和链接使用的颜色。立即生效，只对这台电脑有效。"
                    keywords="颜色 配色 主题色 强调色 蓝色 青绿 紫色 石墨"
                  >
                    <div className="accent-swatches" role="radiogroup" aria-label="主题色">
                      {ACCENTS.map(([key, label]) => (
                        <button
                          key={key}
                          type="button"
                          role="radio"
                          aria-checked={appearance.accent === key}
                          className="accent-swatch"
                          data-swatch={key}
                          onClick={() => setAccent(key)}
                        >
                          <i aria-hidden="true" />
                          {label}
                        </button>
                      ))}
                    </div>
                  </Preference>
                  <Preference
                    title="界面缩放"
                    description="整个界面等比放大或缩小，字太小时可以调大。立即生效，只对这台电脑有效。"
                    htmlFor="ui-zoom"
                    keywords="字体 字号 大小 缩放 放大 看不清"
                  >
                    <select
                      id="ui-zoom"
                      value={zoom}
                      onChange={(event) => setZoom(Number(event.target.value))}
                    >
                      {ZOOM_LEVELS.map((level) => (
                        <option key={level} value={level}>
                          {level}%{level === 100 ? "（默认）" : ""}
                        </option>
                      ))}
                    </select>
                  </Preference>
                  <SettingToggle
                    checked={draft.reduceMotion}
                    onChange={(v) => field("reduceMotion", v)}
                    title="减少界面动效"
                    description="关闭页面切换和导航动画。系统关闭了动画时也会自动生效。"
                  />
                </SettingsGroup>
              </SettingsPanel>
              <SettingsPanel
                id="downloads"
                title="下载"
                hidden={!searching && tab !== "downloads"}
              >
                <SettingsGroup title="速度">
                  <Preference
                    title="下载速度上限"
                    description="填 0 表示不限速。离开输入框或按回车后保存。"
                    htmlFor="speed-limit"
                    keywords="限速 MB/s"
                  >
                    <Input
                      id="speed-limit"
                      inputMode="decimal"
                      value={limit}
                      onChange={(e) => setLimit(e.target.value)}
                      onBlur={saveLimit}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") saveLimit();
                      }}
                      aria-describedby="speed-unit"
                    />
                    <span id="speed-unit" className="muted">
                      MB/s
                    </span>
                  </Preference>
                  <Preference
                    title="同时连接数"
                    description="大于 2 MB 的文件会分成几段同时下载，通常能明显提速。连接太多可能被下载服务临时限制。对之后开始的下载生效。"
                    htmlFor="connections"
                    keywords="分段 多线程 加速 连接"
                  >
                    <select
                      id="connections"
                      value={draft.connections ?? 4}
                      disabled={busy}
                      onChange={(event) =>
                        field("connections", Number(event.target.value))
                      }
                    >
                      {[1, 2, 3, 4, 5, 6, 7, 8].map((count) => (
                        <option key={count} value={count}>
                          {count === 1
                            ? "1（不分段）"
                            : count === 4
                              ? "4（推荐）"
                              : count}
                        </option>
                      ))}
                    </select>
                  </Preference>
                </SettingsGroup>
                <SettingsGroup title="附件选择">
                  <SettingToggle
                    checked={draft.autoMatchPlatform}
                    onChange={(v) => field("autoMatchPlatform", v)}
                    title="自动勾选匹配的附件"
                    description="只有一个附件符合平台、架构和安装版偏好时自动勾选，否则由你选择。"
                  />
                </SettingsGroup>
              </SettingsPanel>
              <SettingsPanel
                id="network"
                title="网络"
                hidden={!searching && tab !== "network"}
              >
                <SettingsGroup title="代理与镜像">
                  <Preference
                    title="网络代理"
                    description="读取版本和下载附件都会使用。改完点下方「保存」，重启应用后生效。"
                    htmlFor="proxy-mode"
                    keywords="网络 代理 直连 系统 HTTP HTTPS"
                  >
                    <select
                      id="proxy-mode"
                      value={draft.network?.proxyMode ?? "direct"}
                      disabled={busy}
                      onChange={(event) =>
                        edit("network", {
                          proxyUrl: "",
                          mirrors: [],
                          ...draft.network,
                          proxyMode: event.target.value as
                            "direct" | "system" | "custom",
                        })
                      }
                    >
                      <option value="direct">不使用代理</option>
                      <option value="system">跟随系统代理</option>
                      <option value="custom">自定义 HTTP / HTTPS 代理</option>
                    </select>
                  </Preference>
                  {draft.network?.proxyMode === "custom" && (
                    <Preference
                      title="代理地址"
                      description="例如 http://127.0.0.1:7890，不支持带账号密码的代理。"
                      htmlFor="proxy-url"
                      keywords="HTTP HTTPS 网络"
                    >
                      <Input
                        id="proxy-url"
                        disabled={busy}
                        value={draft.network.proxyUrl}
                        onChange={(event) =>
                          edit("network", {
                            mirrors: [],
                            ...draft.network,
                            proxyMode: "custom",
                            proxyUrl: event.target.value.trim(),
                          })
                        }
                      />
                    </Preference>
                  )}
                  <Preference
                    title="自定义镜像"
                    description="添加你常用的 GitHub 加速地址，下载时会和内置线路一起测速、自动选最快的。镜像服务能看到你下载的附件地址。改完点下方「保存」，重启应用后生效。"
                    keywords="镜像 加速 线路 mirror 代理"
                    wide
                  >
                    <MirrorList
                      value={draft.network?.mirrors ?? []}
                      disabled={busy}
                      onChange={(mirrors) =>
                        edit("network", {
                          proxyMode: "direct",
                          proxyUrl: "",
                          ...draft.network,
                          mirrors,
                        })
                      }
                    />
                  </Preference>
                  {(networkDirty || restartNeeded) && (
                    <div className="settings-row settings-apply" role="status">
                      {networkDirty ? (
                        <>
                          <span>网络设置有改动，保存后需重启应用生效。</span>
                          <Button
                            variant="ghost"
                            disabled={busy}
                            onClick={() =>
                              edit("network", saved.current?.network)
                            }
                          >
                            放弃
                          </Button>
                          <Button
                            variant="default"
                            disabled={busy || !connected}
                            onClick={saveNetwork}
                          >
                            保存
                          </Button>
                        </>
                      ) : (
                        <>
                          <span>网络设置已保存，重启应用后生效。</span>
                          <Button
                            variant="default"
                            disabled={busy}
                            onClick={() => run(() => request("restart"))}
                          >
                            <RotateCcw size={14} />
                            立即重启
                          </Button>
                        </>
                      )}
                    </div>
                  )}
                </SettingsGroup>
                <SettingsGroup title="线路检测">
                  <Preference
                    title="线路检测"
                    description="下载一小段附件（约 0.5 MB，最多 10 秒），比较 GitHub 直连和加速线路的速度。使用加速线路时，加速服务能看到你下载的附件地址。"
                    keywords="线路 测速 加速 直连 GH-Proxy ghproxy 诊断"
                    wide
                  >
                    <RouteCheck />
                  </Preference>
                </SettingsGroup>
              </SettingsPanel>
              <SettingsPanel
                id="updates"
                title="关注更新"
                hidden={!searching && tab !== "updates"}
              >
                <SettingsGroup title="检查">
                  <SettingToggle
                    checked={draft.autoCheck}
                    onChange={(v) => field("autoCheck", v)}
                    title="自动检查新版本"
                    description="GitHubSP 运行期间按下面的间隔检查。"
                  />
                  <Preference title="检查间隔" htmlFor="check-interval">
                    <select
                      id="check-interval"
                      value={draft.watchIntervalMs}
                      onChange={(e) =>
                        field("watchIntervalMs", Number(e.target.value))
                      }
                    >
                      {[
                        [900000, "15 分钟"],
                        [3600000, "1 小时"],
                        [21600000, "6 小时"],
                        [86400000, "24 小时"],
                      ].map(([value, label]) => (
                        <option key={value} value={value}>
                          {label}
                        </option>
                      ))}
                    </select>
                  </Preference>
                  <SettingToggle
                    checked={draft.watchNotifications}
                    onChange={(v) => field("watchNotifications", v)}
                    title="发现新版本时提醒"
                  />
                </SettingsGroup>
                <SettingsGroup title="新版本">
                  <SettingToggle
                    checked={draft.watchIncludePrerelease}
                    onChange={(v) => field("watchIncludePrerelease", v)}
                    title="包含预发布版本"
                  />
                  <SettingToggle
                    checked={draft.watchAutoDownload}
                    onChange={(v) => field("watchAutoDownload", v)}
                    title="自动下载新版本"
                    description="符合规则的附件会自动加入下载队列。"
                  />
                  <Preference title="自动下载附件" htmlFor="asset-mode">
                    <select
                      id="asset-mode"
                      value={draft.watchAssetMode}
                      onChange={(e) =>
                        field("watchAssetMode", Number(e.target.value))
                      }
                    >
                      <option value={0}>优先安装包</option>
                      <option value={1}>全部附件</option>
                      <option value={2}>不下载附件</option>
                    </select>
                  </Preference>
                  <Preference
                    title="附件名匹配规则"
                    description="每个项目也可以在「我的软件 → 项目设置」里单独设定。"
                    htmlFor="global-pattern"
                  >
                    <Input
                      id="global-pattern"
                      placeholder="*windows*x64*.zip"
                      value={draft.watchAssetPattern}
                      onChange={(e) =>
                        edit("watchAssetPattern", e.target.value)
                      }
                      onBlur={savePattern}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") savePattern();
                      }}
                    />
                  </Preference>
                </SettingsGroup>
              </SettingsPanel>
              <SettingsPanel
                id="data"
                title="数据与备份"
                hidden={!searching && tab !== "data"}
              >
                <SettingsGroup title="数据">
                  <Preference
                    title="当前数据目录"
                    description={info?.directory}
                    keywords="存储 路径 打开目录"
                  >
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        run(() => request("openDirectory", { taskId: null }))
                      }
                    >
                      <FolderOpen size={14} />
                      打开目录
                    </Button>
                  </Preference>
                  <Preference
                    title="导出备份"
                    description="把关注、下载记录和设置导出成一个备份文件。"
                    keywords="导出 SQLite 备份"
                  >
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        run(async () => {
                          const path = await request<string | null>(
                            "exportBackup",
                          );
                          if (path)
                            notify(
                              `备份已导出并通过完整性检查：${path}`,
                              "success",
                            );
                        })
                      }
                    >
                      导出备份
                    </Button>
                  </Preference>
                  <Preference
                    title="迁移数据目录"
                    keywords="将原目录作为备份保留 新目录 迁移并重启"
                    htmlFor="migration-directory"
                    description={
                      networkDirty
                        ? "请先保存或放弃网络设置的改动。"
                        : "把应用数据搬到新文件夹，下载好的文件留在原处。"
                    }
                    wide
                  >
                    <div className="input-row">
                      <Input
                        id="migration-directory"
                        value={target}
                        disabled={!info?.migrationAllowed || busy}
                        onChange={(e) => setTarget(e.target.value)}
                        placeholder="选择一个新的空文件夹"
                      />
                      <Button
                        disabled={!info?.migrationAllowed || busy}
                        aria-label="浏览迁移目标"
                        onClick={() =>
                          run(async () => {
                            const path = await request<string | null>(
                              "chooseDirectory",
                              { initial: target || null },
                            );
                            if (path && active.current) setTarget(path);
                          })
                        }
                      >
                        <FolderOpen size={15} />
                      </Button>
                    </div>
                    <div className="settings-migration-actions">
                      <label className="checkbox-line">
                        <input
                          type="checkbox"
                          checked={keep}
                          onChange={(e) => setKeep(e.target.checked)}
                          disabled={busy || !info?.migrationAllowed}
                        />
                        将原目录作为备份保留
                      </label>
                      <Button
                        disabled={
                          !info?.migrationAllowed ||
                          busy ||
                          !target.trim() ||
                          networkDirty
                        }
                        onClick={() =>
                          run(async () => {
                            const revision = settingsEpoch.current;
                            const inspected = await request<{
                              directory: string;
                              state: string;
                            }>("inspectMigration", {
                              directory: target.trim(),
                            });
                            if (
                              !active.current ||
                              !visible.current ||
                              revision !== settingsEpoch.current
                            )
                              return;
                            if (inspected.state === "missing") {
                              if (
                                !(await confirm(
                                  "创建迁移目标目录？",
                                  inspected.directory,
                                  "创建目录",
                                )) ||
                                !active.current ||
                                !visible.current ||
                                revision !== settingsEpoch.current
                              )
                                return;
                              await request("createDirectory", {
                                directory: inspected.directory,
                              });
                            }
                            if (
                              !active.current ||
                              !visible.current ||
                              revision !== settingsEpoch.current ||
                              !(await confirm(
                                "迁移数据并重启？",
                                `下载将保存并暂停。目标目录：${inspected.directory}\n${keep ? "原目录保留为备份。" : "迁移成功后清理原目录中已校验的应用数据。"}下载文件和续传分片保留在原位置。`,
                                "迁移并重启",
                              ))
                            )
                              return;
                            await request("migrate", {
                              directory: inspected.directory,
                              keepBackup: keep,
                            });
                          })
                        }
                      >
                        迁移并重启
                      </Button>
                    </div>
                    {!info?.migrationAllowed && (
                      <p className="settings-state-note">
                        当前是测试或随身数据模式，不能迁移数据目录。
                      </p>
                    )}
                    {info?.locationState?.message && (
                      <p className="settings-state-note">
                        {info.locationState.message}
                      </p>
                    )}
                  </Preference>
                </SettingsGroup>
              </SettingsPanel>
              <SettingsPanel
                id="about"
                title="关于"
                hidden={!searching && tab !== "about"}
              >
                <SettingsGroup title="应用">
                  <Preference
                    title="GitHubSP"
                    description={`版本 ${info?.version ?? "0.4.1"}`}
                    keywords="检查应用更新 版本"
                  >
                    <img
                      className="settings-app-icon"
                      src={appIcon}
                      width="32"
                      height="32"
                      alt=""
                    />
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        run(async () => {
                          const revision = settingsEpoch.current;
                          const result =
                            await request<UpdateResult>("checkUpdate");
                          if (
                            active.current &&
                            visible.current &&
                            revision === settingsEpoch.current
                          )
                            setUpdate(result);
                        })
                      }
                    >
                      检查应用更新
                    </Button>
                  </Preference>
                  <SettingToggle
                    checked={draft.logEnabled}
                    onChange={(v) => field("logEnabled", v)}
                    title="记录本地诊断日志"
                    description="仅保存在本地，不记录令牌和剪贴板内容。"
                  />
                  <Preference
                    title="诊断信息"
                    description="复制诊断信息以便排查应用问题。"
                  >
                    <Button
                      size="sm"
                      disabled={busy}
                      onClick={() =>
                        run(async () => {
                          const text = await request<string>("diagnostics");
                          await request("copy", { text });
                          notify("诊断信息已复制", "success");
                        })
                      }
                    >
                      <Copy size={14} />
                      复制诊断信息
                    </Button>
                  </Preference>
                  <Preference
                    title="开源许可"
                    description="查看本应用使用的开源项目和许可。"
                    keywords="MIT 协议 版权 关于与许可"
                  >
                    <Button size="sm" onClick={() => setAbout(true)}>
                      关于与许可
                      <ExternalLink size={13} />
                    </Button>
                  </Preference>
                </SettingsGroup>
              </SettingsPanel>
              <p className="settings-empty" role="status">
                没有找到匹配的设置，试试“代理”“备份”或“更新”。
              </p>
            </div>
          </SettingsQuery.Provider>
        </div>
      )}
      <Modal
        open={!!update}
        onClose={() => setUpdate(null)}
        wide
        title="应用更新"
        description={update?.message}
        footer={
          <>
            <Button onClick={() => setUpdate(null)}>关闭</Button>
            {update?.latestRelease && (
              <Button
                variant="default"
                onClick={() => {
                  setUpdate(null);
                  openRepository(update.latestRelease!.url);
                }}
              >
                查看版本与下载附件
                <ExternalLink size={14} />
              </Button>
            )}
          </>
        }
      >
        <p>
          当前版本：{update?.current}　最新版本：{update?.latest || "尚未取得"}
        </p>
        {update?.notes && <Markdown>{update.notes}</Markdown>}
      </Modal>
      <Modal
        open={about}
        onClose={() => setAbout(false)}
        title="关于 GitHubSP"
        description="公开 GitHub Release 附件下载工作台"
        footer={<Button onClick={() => setAbout(false)}>关闭</Button>}
      >
        <p>下载 GitHub Release 附件，管理下载记录与项目更新。</p>
        <p className="form-note">
          界面适配了 magpie 的页面与导航，以及 cc-switch 的基础控件和 Motrix
          的任务进度组件，保留相关版权与 MIT
          许可。完整来源与依赖声明随安装版和免安装版提供。
        </p>
        <details>
          <summary>完整第三方声明</summary>
          <pre className="license-text">{notices}</pre>
        </details>
        <Button
          variant="ghost"
          onClick={() =>
            run(() =>
              request("openLink", {
                url: "https://github.com/lkuliuying/githubsp",
              }),
            )
          }
        >
          项目主页
          <ExternalLink size={13} />
        </Button>
      </Modal>
    </section>
  );
}
