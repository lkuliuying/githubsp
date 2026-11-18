import { useEffect, useRef, useState } from "react";
import { ArrowLeftRight } from "lucide-react";
import { request, notify } from "@/lib/api";
import { type WatchlistPreview } from "@/lib/model";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Modal } from "./ui/dialog";
import { useJob } from "./common";

export function WatchlistTransfer() {
  const [open, setOpen] = useState(false);
  const [mode, setMode] = useState<"export" | "import">("export");
  const [text, setText] = useState("");
  const [directory, setDirectory] = useState("");
  const [policy, setPolicy] = useState("keep");
  const [preview, setPreview] = useState<WatchlistPreview | null>(null);
  const previewRef = useRef<HTMLDivElement>(null);
  useEffect(() => { if (preview) previewRef.current?.scrollIntoView({ block: "start" }); }, [preview]);
  const { busy, run, active } = useJob();
  const exportText = async () => {
    const value = await request("watchlistExport");
    if (active.current) setText(JSON.stringify(value, null, 2));
  };
  const changed = preview?.items.filter(item => item.action !== "keep").length ?? 0;
  return <><Button onClick={() => { setOpen(true); setMode("export"); setPreview(null); run(exportText); }} aria-label="导入或导出关注清单"><ArrowLeftRight size={15} /><span className="collapsible-label">关注清单</span></Button>
    <Modal open={open} onClose={() => setOpen(false)} busy={busy} title="可迁移关注清单" description="把关注的项目和偏好带到另一台电脑。要保留全部数据，请用「设置 → 数据与备份」。"
      footer={mode === "export" ? <><Button disabled={busy || !text} onClick={() => run(async () => { await request("copy", { text }); notify("关注清单已复制", "success"); })}>复制 JSON</Button><Button disabled={busy} onClick={() => run(async () => { const path = await request("watchlistExportFile"); if (path) notify("关注清单已导出", "success"); })}>导出 JSON 文件</Button></> : <>
        <Button disabled={busy || !text.trim() || !directory.trim()} onClick={() => run(async () => { const value = await request<WatchlistPreview>("watchlistPreview", { manifest: text, directory, policy }); if (active.current) setPreview(value); })}>预览合并</Button>
        <Button disabled={busy || !preview || !changed} onClick={() => run(async () => {
          if (!preview) return;
          try { await request("watchlistImport", { manifest: text, directory: preview.directory, policy, fingerprint: preview.fingerprint }); if (active.current) { setOpen(false); notify("关注清单已导入", "success"); } }
          catch (error) { setPreview(null); throw error; }
        })}>确认导入{preview ? `（${changed} 项）` : ""}</Button>
      </>}>
      <div className="watchlist-modes"><Button disabled={busy} variant={mode === "export" ? "default" : "outline"} onClick={() => { setMode("export"); setPreview(null); run(exportText); }}>导出</Button><Button disabled={busy} variant={mode === "import" ? "default" : "outline"} onClick={() => { setMode("import"); setText(""); setPreview(null); }}>导入</Button></div>
      {mode === "export" ? <p className="update-intro">清单只包含项目、偏好、分组和标签，不含本机路径和下载记录。</p> : <>
        <label className="field-label" htmlFor="watchlist-file">选择 JSON 文件，或在下方粘贴</label><input id="watchlist-file" type="file" accept="application/json,.json" disabled={busy} onChange={event => { const file = event.target.files?.[0]; if (file) run(async () => { if (file.size > 1_048_576) throw Error("关注清单超过 1 MiB"); const value = await file.text(); if (active.current) { setText(value); setPreview(null); } }); }} />
      </>}
      <label className="field-label" htmlFor="watchlist-json">关注清单 JSON</label><textarea id="watchlist-json" className="watchlist-json" rows={10} value={text} readOnly={mode === "export"} disabled={busy} onChange={event => { setText(event.target.value); setPreview(null); }} />
      {mode === "import" && <>
        <label className="field-label" htmlFor="watchlist-directory">本机下载根目录</label><div className="watchlist-directory"><Input id="watchlist-directory" value={directory} onChange={event => { setDirectory(event.target.value); setPreview(null); }} /><Button disabled={busy} onClick={() => run(async () => { const value = await request<string | null>("chooseDirectory", { initial: directory || null }); if (value && active.current) { setDirectory(value); setPreview(null); } })}>选择目录</Button></div>
        <label className="field-label" htmlFor="watchlist-policy">重复与冲突处理</label><select id="watchlist-policy" value={policy} disabled={busy} onChange={event => { setPolicy(event.target.value); setPreview(null); }}><option value="keep">合并并保留已有项目</option><option value="replace">覆盖已有规则与标签</option></select>
        <p className="update-intro">新增和覆盖的项目会下载到上面选的文件夹，保留的项目不变。新关注的项目从下一个版本开始提醒。</p>
        {preview && <div className="watchlist-preview" ref={previewRef} aria-label="关注清单导入预览"><strong>将变更 {changed} 个项目 · {preview.directory}</strong>{preview.items.map(item => <div key={item.repository}><span>{item.repository}</span><small>{{new:"新增",duplicate:"重复",conflict:"冲突"}[item.state] ?? item.state} · {{add:"添加",keep:"保留现有",replace:"覆盖"}[item.action] ?? item.action} · 自动下载{item.autoDownload ? "开启" : "关闭"}</small></div>)}</div>}
      </>}
    </Modal></>;
}
