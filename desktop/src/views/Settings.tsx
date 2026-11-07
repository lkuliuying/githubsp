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
import { Copy, ExternalLink, FolderOpen, Save, Search, X } from "lucide-react";
import { notify, request, useWorkspace } from "@/lib/api";
import {
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
import notices from "../../THIRD_PARTY_NOTICES.md?raw";

// 改编自 magpie 的 set-tabs、prefs、segs 与设置搜索（MIT，固定来源见第三方声明）。
const settingsTabs = [
  ["general", "通用"],
  ["downloads", "下载"],
  ["updates", "收藏更新"],
  ["data", "数据与备份"],
  ["about", "关于"],
] as const;
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
      <div className="settings-preferences">{children}</div>
    </section>
  );
}

let savedDraft: SettingsModel | null = null;
let savedLimit: string | null = null;
let baseline = "";
export function hasUnsavedSettings() {
  return (
    savedDraft !== null &&
    (settingsFingerprint(savedDraft) !== baseline ||
      Number(savedLimit) !==
        Number(((savedDraft.limitKib * 1024) / 1_000_000).toFixed(6)))
  );
}
export function Settings() {
  const visible = usePageActiveRef();
  const { info, snapshot, connected } = useWorkspace();
  const [draft, setDraft] = useState<SettingsModel | null>(
    savedDraft ?? snapshot?.settings ?? null,
  );
  const [limit, setLimit] = useState(
    savedLimit ??
      String(
        Number(
          (((snapshot?.settings.limitKib ?? 0) * 1024) / 1_000_000).toFixed(6),
        ),
      ),
  );
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
  useEffect(() => {
    if (!draft && snapshot) {
      setDraft(snapshot.settings);
      setLimit(
        String(
          Number(((snapshot.settings.limitKib * 1024) / 1_000_000).toFixed(6)),
        ),
      );
    }
  }, [draft, snapshot]);
  useEffect(() => {
    if (draft && !baseline)
      baseline = settingsFingerprint(snapshot?.settings ?? draft);
    savedDraft = draft;
    savedLimit = limit;
  }, [draft, limit, snapshot?.settings]);
  const dirty =
    !!draft &&
    (settingsFingerprint(draft) !== settingsFingerprint(snapshot?.settings) ||
      Number(limit) !==
        Number(((draft.limitKib * 1024) / 1_000_000).toFixed(6)));
  function field<K extends keyof SettingsModel>(
    key: K,
    value: SettingsModel[K],
  ) {
    setDraft((old) => (old ? { ...old, [key]: value } : old));
  }
  const save = () =>
    run(async () => {
      if (!draft) return;
      const settings = {
        ...draft,
        limitKib: parseLimit(limit, snapshot!.settings.limitKib),
      };
      await request("saveSettings", { settings });
      if (!active.current) return;
      setDraft(settings);
      savedDraft = settings;
      baseline = settingsFingerprint(settings);
      setLimit(
        String(Number(((settings.limitKib * 1024) / 1_000_000).toFixed(6))),
      );
      notify("设置已保存", "success");
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
    const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const index = settingsTabs.findIndex(([key]) => key === tab);
    const nextIndex =
      event.key === "Home"
        ? 0
        : event.key === "End"
          ? settingsTabs.length - 1
          : (index +
              (event.key === "ArrowRight" ? 1 : -1) +
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
        <h1>
          设置{dirty && <span className="unsaved-dot" title="有未保存更改" />}
        </h1>
        <div className="header-actions">
          {dirty && (
            <Button
              variant="ghost"
              disabled={busy}
              onClick={() => {
                setDraft(snapshot!.settings);
                setLimit(
                  String(
                    Number(
                      (
                        (snapshot!.settings.limitKib * 1024) /
                        1_000_000
                      ).toFixed(6),
                    ),
                  ),
                );
              }}
            >
              放弃更改
            </Button>
          )}
          <Button
            variant="default"
            disabled={!connected || busy || !dirty}
            onClick={save}
          >
            <Save size={15} />
            {busy ? "正在处理…" : "保存设置"}
          </Button>
        </div>
      </PageHeader>
      {draft && (
        <div className="settings-shell">
          <div className="settings-toolbar">
            <div
              className="settings-tabs"
              role="tablist"
              aria-label="设置分类"
              onKeyDown={tabKeys}
            >
              {settingsTabs.map(([key, label]) => (
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
                  {label}
                </button>
              ))}
            </div>
            <label className="settings-search">
              <Search size={14} aria-hidden="true" />
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
                  <X size={13} />
                </button>
              )}
            </label>
          </div>
          <SettingsQuery.Provider value={query}>
            <div className="settings-panels">
              <SettingsPanel
                id="general"
                title="窗口与通知"
                hidden={!searching && tab !== "general"}
              >
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
                  checked={draft.reduceMotion}
                  onChange={(v) => field("reduceMotion", v)}
                  title="减少界面动效"
                  description="立即切换页面和导航；系统关闭动画时也会生效。"
                />
              </SettingsPanel>
              <SettingsPanel
                id="downloads"
                title="下载偏好"
                hidden={!searching && tab !== "downloads"}
              >
                <Preference
                  title="下载速度上限"
                  description="0 表示不限速。队列按顺序串行处理。"
                  htmlFor="speed-limit"
                  keywords="限速 MB/s"
                >
                  <Input
                    id="speed-limit"
                    inputMode="decimal"
                    value={limit}
                    onChange={(e) => setLimit(e.target.value)}
                    disabled={busy}
                    aria-describedby="speed-unit"
                  />
                  <span id="speed-unit" className="muted">
                    MB/s
                  </span>
                </Preference>
                <SettingToggle
                  checked={draft.autoMatchPlatform}
                  onChange={(v) => field("autoMatchPlatform", v)}
                  title="自动勾选当前平台附件"
                  description="读取版本时优先选择 Windows 附件，请继续确认架构。"
                />
              </SettingsPanel>
              <SettingsPanel
                id="updates"
                title="收藏更新"
                hidden={!searching && tab !== "updates"}
              >
                <SettingToggle
                  checked={draft.autoCheck}
                  onChange={(v) => field("autoCheck", v)}
                  title="自动检查收藏更新"
                  description="仅在 GitHubSP 运行期间检查，退出后停止。"
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
                <SettingToggle
                  checked={draft.watchIncludePrerelease}
                  onChange={(v) => field("watchIncludePrerelease", v)}
                  title="包含预发布版本"
                />
                <SettingToggle
                  checked={draft.watchAutoDownload}
                  onChange={(v) => field("watchAutoDownload", v)}
                  title="自动下载新版本"
                  description="命中的附件将加入现有下载队列。"
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
                  description="单个收藏可以设置独立关注规则。"
                  htmlFor="global-pattern"
                >
                  <Input
                    id="global-pattern"
                    placeholder="*windows*x64*.zip"
                    value={draft.watchAssetPattern}
                    onChange={(e) => field("watchAssetPattern", e.target.value)}
                  />
                </Preference>
              </SettingsPanel>
              <SettingsPanel
                id="data"
                title="数据与备份"
                hidden={!searching && tab !== "data"}
              >
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
                  description="导出数据库并检查备份文件的完整性。"
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
                    导出 SQLite 备份
                  </Button>
                </Preference>
                <Preference
                  title="迁移数据目录"
                  keywords="将原目录作为备份保留 新目录 迁移并重启"
                  htmlFor="migration-directory"
                  description="迁移不移动下载成品和续传分片。迁移前请先保存设置。"
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
                        dirty
                      }
                      onClick={() =>
                        run(async () => {
                          const revision = settingsEpoch.current;
                          const inspected = await request<{
                            directory: string;
                            state: string;
                          }>("inspectMigration", { directory: target.trim() });
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
                      当前为隔离运行或随身数据模式，不修改正式数据位置。
                    </p>
                  )}
                  {info?.locationState?.message && (
                    <p className="settings-state-note">
                      {info.locationState.message}
                    </p>
                  )}
                </Preference>
              </SettingsPanel>
              <SettingsPanel
                id="about"
                title="应用与诊断"
                hidden={!searching && tab !== "about"}
              >
                <Preference
                  title="GitHubSP"
                  description={`版本 ${info?.version ?? "0.3.0"}`}
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
                  description="magpie、cc-switch、Motrix 的来源与第三方依赖声明。"
                  keywords="MIT 协议 版权 关于与许可"
                >
                  <Button size="sm" onClick={() => setAbout(true)}>
                    关于与许可
                    <ExternalLink size={13} />
                  </Button>
                </Preference>
              </SettingsPanel>
              <p className="settings-empty" role="status">
                未找到匹配的设置，试试“下载”“备份”或“更新”。
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
