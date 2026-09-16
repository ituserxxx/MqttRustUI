// 后端调用封装：把 Tauri invoke / listen 收敛到 api 层。
// 业务组件只允许调用本文件，不得直接 invoke，保持与桌面端解耦（将来换 Web Worker 只改这里）。
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  AppSettings,
  BackpressureEvent,
  Connection,
  MessageBatch,
  Qos,
  StateChanged,
  StatsEvent,
  StoredMessage,
} from '../types'

export const api = {
  connect: (connectionId: string) => invoke<void>('connect', { connectionId }),
  disconnect: (connectionId: string) => invoke<void>('disconnect', { connectionId }),
  subscribe: (connectionId: string, filter: string, qos: Qos) =>
    invoke<void>('subscribe', { connectionId, filter, qos }),
  unsubscribe: (connectionId: string, filter: string) =>
    invoke<void>('unsubscribe', { connectionId, filter }),
  publish: (
    connectionId: string,
    topic: string,
    payloadBase64: string,
    qos: Qos,
    retain: boolean,
  ) => invoke<void>('publish', { connectionId, topic, payloadBase64, qos, retain }),
  getMessages: (connectionId: string, limit: number) =>
    invoke<StoredMessage[]>('get_messages', { connectionId, limit }),
  clearMessages: (connectionId: string) =>
    invoke<void>('clear_messages', { connectionId }),
  listConnections: () => invoke<Connection[]>('list_connections'),
  getConnection: (id: string) => invoke<Connection>('get_connection', { id }),
  saveConnection: (connection: Connection, password?: string) =>
    invoke<void>('save_connection', { connection, password }),
  deleteConnection: (id: string) => invoke<void>('delete_connection', { id }),
  getSettings: () => invoke<AppSettings>('get_settings'),
  saveSettings: (settings: AppSettings) =>
    invoke<void>('save_settings', { settings }),
  exportConfig: () => invoke<string>('export_config'),
  importConfig: (json: string) => invoke<void>('import_config', { json }),
  disconnectAll: () => invoke<void>('disconnect_all'),
}

export interface EventHandlers {
  onState?: (p: StateChanged) => void
  onBatch?: (p: MessageBatch) => void
  onStats?: (p: StatsEvent) => void
  onBackpressure?: (p: BackpressureEvent) => void
}

/** 注册四类后端事件监听，返回取消函数。 */
export async function listenEvents(h: EventHandlers): Promise<UnlistenFn> {
  const a = await listen<StateChanged>('mqttkit:state', (e) => h.onState?.(e.payload))
  const b = await listen<MessageBatch>('mqttkit:message_batch', (e) => h.onBatch?.(e.payload))
  const c = await listen<StatsEvent>('mqttkit:stats', (e) => h.onStats?.(e.payload))
  const d = await listen<BackpressureEvent>('mqttkit:backpressure', (e) =>
    h.onBackpressure?.(e.payload),
  )
  return () => {
    a()
    b()
    c()
    d()
  }
}

/** 把 UTF-8 文本编码为 base64（与 Rust 端 decode_base64 对应）。 */
export function encodeBase64(text: string): string {
  const bytes = new TextEncoder().encode(text)
  let bin = ''
  for (const b of bytes) bin += String.fromCharCode(b)
  return btoa(bin)
}

/** 把 base64 解码为 UTF-8 文本（失败返回原始串）。 */
export function decodeBase64(b64: string): string {
  try {
    const bin = atob(b64)
    const bytes = new Uint8Array(bin.length)
    for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i)
    return new TextDecoder().decode(bytes)
  } catch {
    return b64
  }
}
