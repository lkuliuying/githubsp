window.__ModuleLoader__.load({
  id: 'dsh-githubsp',
  factory: require => {
    const { createElement: h, useEffect, useRef, useState, useSyncExternalStore } = require('react');
    const { Button } = require('@deepseek-ai/dsh-client-ui-primitives');
    const dictionaries = {
      zh: {
        summary: '通过本机 GitHubSP 下载公开 Release、复用软件文件和管理关注更新。',
        directory: 'GitHubSP 数据目录',
        help: '填写 GitHubSP「设置 → 数据与备份」显示的实际数据目录。这不是下载保存目录。',
        fallback: '留空时读取启动 Harness 时的 GITHUBSP_DATA_DIR 环境变量。',
        save: '保存设置', reload: '重新读取', saving: '正在保存…', saved: '设置已保存，后续工具调用使用新目录。',
        invalid: '请输入 Windows 绝对路径，或留空使用环境变量。',
        refused: '宿主未接受保存，请重新读取设置后重试。',
        changed: '设置已在其他位置修改，请重新读取后再保存。',
        loading: '正在读取设置…', unavailable: '设置暂不可用，请确认插件已启用后重新打开。',
        readonly: '当前连接没有修改设置的权限。',
        boundary: 'GitHubSP 需要保持运行，下载允许目录在 GitHubSP 的 Agent 接入设置中管理。停用插件不会取消已接受的下载。',
      },
      en: {
        summary: 'Download public releases, reuse local software files, and track updates through GitHubSP.',
        directory: 'GitHubSP data directory',
        help: 'Use the actual data directory shown in GitHubSP Settings → Data and backup, not the download destination.',
        fallback: 'Leave blank to use GITHUBSP_DATA_DIR from the Harness process environment.',
        save: 'Save settings', reload: 'Reload settings', saving: 'Saving…', saved: 'Saved. Future tool calls use the new directory.',
        invalid: 'Enter an absolute Windows path or leave blank to use the environment variable.',
        refused: 'The host did not accept the write. Reload settings and try again.',
        changed: 'Settings changed elsewhere. Reload before saving.',
        loading: 'Loading settings…', unavailable: 'Settings are unavailable. Enable the plugin and reopen this page.',
        readonly: 'This connection cannot modify settings.',
        boundary: 'Keep GitHubSP running. Manage allowed download directories in its Agent settings. Disabling this plugin does not cancel accepted downloads.',
      },
    };
    function SettingsPage({ form, t, view }) {
      const state = useSyncExternalStore(listener => form.subscribe(listener), () => form.getSnapshot());
      const [draft, setDraft] = useState('');
      const [baseline, setBaseline] = useState('');
      const [dirty, setDirty] = useState(false);
      const [busy, setBusy] = useState(false);
      const [error, setError] = useState('');
      const [message, setMessage] = useState('');
      const alive = useRef(true);
      const current = state.value?.dataDirectory ?? '';
      useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);
      useEffect(() => {
        if (!dirty && state.status === 'ready') { setDraft(current); setBaseline(current); }
      }, [current, dirty, state.status]);
      const reload = () => {
        setDraft(current); setBaseline(current); setDirty(false); setError(''); setMessage('');
      };
      const save = async () => {
        const value = draft.trim();
        if (!/^(?:$|[a-z]:[\\/]|\\\\)/i.test(value)) { setError(t('invalid')); return; }
        if (form.getSnapshot().value?.dataDirectory !== baseline) { setError(t('changed')); return; }
        setBusy(true); setError(''); setMessage('');
        try {
          // ConfigForms 以 false 表示被拒绝；只有实际接受写入才显示成功。
          if (!await form.set('dataDirectory', value)) throw Error(t('refused'));
          if (alive.current) { setDraft(value); setBaseline(value); setDirty(false); setMessage(t('saved')); }
        } catch (error) { if (alive.current) setError(error.message || String(error)); }
        finally { if (alive.current) setBusy(false); }
      };
      if (view === 'summary') return h('span', null, t('summary'));
      if (state.status !== 'ready') return h('p', { role: 'status' }, t(state.status === 'loading' || state.status === 'idle' ? 'loading' : 'unavailable'));
      return h('section', { className: 'githubsp-settings', 'aria-label': 'GitHubSP' },
        h('label', { className: 'githubsp-label', htmlFor: 'githubsp-data-directory' }, t('directory')),
        h('p', { className: 'githubsp-help', id: 'githubsp-directory-help' }, t('help')),
        h('input', {
          id: 'githubsp-data-directory', type: 'text', value: draft, autoComplete: 'off', spellCheck: false,
          'aria-describedby': 'githubsp-directory-help', disabled: busy || !state.writable,
          onChange: event => { setDraft(event.target.value); setDirty(true); setError(''); setMessage(''); },
        }),
        h('p', { className: 'githubsp-help' }, t('fallback')),
        !state.writable && h('p', { role: 'status' }, t('readonly')),
        h('div', { className: 'githubsp-actions' },
          h(Button, { disabled: busy, onClick: reload }, t('reload')),
          h(Button, { disabled: busy || !dirty || !state.writable, onClick: save }, t(busy ? 'saving' : 'save')),
        ),
        error && h('p', { role: 'alert', className: 'githubsp-error' }, error),
        message && h('p', { role: 'status' }, message),
        h('p', { className: 'githubsp-boundary' }, t('boundary')),
      );
    }
    return {
      inject: ['slots', 'locale', 'configForms'],
      apply(ctx) {
        ctx.effect(() => {
          const style = document.createElement('style');
          style.textContent = '.githubsp-settings{max-width:720px;color:var(--dsw-alias-label-primary);padding:8px 0 20px}.githubsp-label{font-size:14px;font-weight:600}.githubsp-help,.githubsp-boundary{font-size:13px;line-height:1.65;color:var(--dsw-alias-label-secondary);margin:8px 0 12px}.githubsp-settings input{display:block;width:100%;box-sizing:border-box;border:1px solid var(--dsw-alias-border-l2);border-radius:8px;background:var(--dsw-alias-bg-layer-2);color:inherit;padding:10px 12px;font:inherit}.githubsp-settings input:focus-visible{outline:2px solid var(--dsw-alias-state-business-primary);outline-offset:2px}.githubsp-actions{display:flex;justify-content:flex-end;flex-wrap:wrap;gap:8px;margin:18px 0}.githubsp-error{color:var(--dsw-alias-state-danger-primary,#b42318)}.githubsp-boundary{border-top:1px solid var(--dsw-alias-border-l2);padding-top:16px;margin-top:20px}';
          document.head.appendChild(style);
          return () => style.remove();
        }, 'githubsp: settings styles');
        ctx.effect(() => ctx.locale.register('githubsp', dictionaries), 'githubsp: settings locale');
        const form = ctx.configForms.get('githubsp');
        ctx.effect(() => ctx.configForms.whileServed(['githubsp'], () => ctx.slots.inject('plugins.bundle.config', () => ctx.slots.register({
          name: 'plugins.bundle.config', key: 'dsh-githubsp', locale: 'githubsp', inject: () => ({ form }),
        }, SettingsPage))), 'githubsp: settings page');
      },
    };
  },
});
