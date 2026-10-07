import { beforeEach, describe, expect, it, vi } from 'vitest'
import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { downloadsApi } from './downloads'

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn(), isTauri: vi.fn() }))
vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }))

beforeEach(() => { vi.resetAllMocks() })

describe('系统目录选择边界', () => {
  it('校验初始目录和所选目录，返回规范化后的存在目录', async () => {
    vi.mocked(invoke).mockResolvedValueOnce({ directory: 'F:\\原目录', state: 'existing' })
      .mockResolvedValueOnce({ directory: 'F:\\选中目录', state: 'existing' })
    vi.mocked(open).mockResolvedValue('F:\\选中目录\\.')
    expect(await downloadsApi.chooseDirectory('F:\\原目录')).toBe('F:\\选中目录')
    expect(open).toHaveBeenCalledExactlyOnceWith({ directory: true, multiple: false, title: '选择下载保存目录', defaultPath: 'F:\\原目录' })
    expect(invoke).toHaveBeenNthCalledWith(2, 'inspect_directory', { directory: 'F:\\选中目录\\.' })
    expect(invoke).not.toHaveBeenCalledWith('create_directory', expect.anything())
  })

  it.each(['missing', 'inaccessible', 'empty'])('初始位置为 %s 时允许从系统默认位置重新选择', async state => {
    if (state === 'inaccessible') vi.mocked(invoke).mockRejectedValueOnce('保存目录无法访问')
    else if (state === 'missing') vi.mocked(invoke).mockResolvedValueOnce({ directory: 'F:\\missing', state: 'missing' })
    vi.mocked(open).mockResolvedValue(null)
    expect(await downloadsApi.chooseDirectory(state === 'empty' ? '' : 'F:\\missing')).toBeNull()
    expect(open).toHaveBeenCalledWith(expect.objectContaining({ defaultPath: undefined, directory: true, multiple: false }))
  })

  it('不接受已经消失的目录，也不会代建目录', async () => {
    vi.mocked(open).mockResolvedValue('F:\\missing')
    vi.mocked(invoke).mockResolvedValue({ directory: 'F:\\missing', state: 'missing' })
    await expect(downloadsApi.chooseDirectory('')).rejects.toThrow('所选目录已不存在')
    expect(invoke).toHaveBeenCalledExactlyOnceWith('inspect_directory', { directory: 'F:\\missing' })
  })

  it('所选路径为文件或无法访问时将后端错误返回界面', async () => {
    vi.mocked(open).mockResolvedValue('F:\\file.txt')
    vi.mocked(invoke).mockRejectedValue('保存位置或其父路径不是目录')
    await expect(downloadsApi.chooseDirectory('')).rejects.toBe('保存位置或其父路径不是目录')
  })

  it('空的选择结果不能传递给后端', async () => {
    vi.mocked(open).mockResolvedValue('')
    await expect(downloadsApi.chooseDirectory('')).rejects.toThrow('请选择一个已存在的目录')
    expect(invoke).not.toHaveBeenCalled()
  })

  it('显式创建接口仅传递指定目录，不创建任务', async () => {
    vi.mocked(invoke).mockResolvedValue('F:\\新目录')
    expect(await downloadsApi.createDirectory('F:\\新目录')).toBe('F:\\新目录')
    expect(invoke).toHaveBeenCalledExactlyOnceWith('create_directory', { directory: 'F:\\新目录' })
  })
})
