// 与 Rust 端 mqttkit_ipc::model 对应的前端类型（字段命名保持 snake_case，便于直接传递）。

export type ProtocolVersion = 'v311' | 'v50'
export type Qos = 'at_most_once' | 'at_least_once' | 'exactly_once'
export type ConnectionState =
  | 'idle'
  | 'connecting'
  | 'connected'
  | 'reconnecting'
  | 'failed'
  | 'disconnecting'

export type TlsMode = 'none' | 'tls'

export interface ClientAuth {
  cert: string
  key: string
}

export interface TlsConfig {
  mode: TlsMode
  ca?: string | null
  client_auth?: ClientAuth | null
  verify_hostname: boolean
}

export interface ProxyConfig {
  url: string
}

export interface LastWill {
  topic: string
  payload_base64: string
  qos: Qos
  retain: boolean
}

export interface Mqtt5Props {
  session_expiry_interval?: number | null
  maximum_packet_size?: number | null
  topic_alias_max?: number | null
  user_properties: Record<string, string>
}

export interface Subscription {
  filter: string
  qos: Qos
  color?: string | null
  enabled: boolean
}

export interface Connection {
  id: string
  name: string
  protocol: string
  host: string
  port: number
  path?: string | null
  client_id: string
  username: string
  credential_ref?: string | null
  mqtt_version: ProtocolVersion
  clean_session: boolean
  keep_alive: number
  tls: TlsConfig
  proxy?: ProxyConfig | null
  last_will?: LastWill | null
  properties?: Mqtt5Props | null
  auto_connect: boolean
  subscriptions: Subscription[]
}

export interface StoredMessage {
  id: string
  connection_id: string
  topic: string
  payload_base64: string
  preview?: string | null
  size: number
  qos: Qos
  retain: boolean
  timestamp: number
}

export interface TrafficStats {
  received: number
  sent: number
  recv_rate: number
  send_rate: number
}

export interface AppSettings {
  theme: string
  locale: string
  message_buffer: number
  flush_interval_ms: number
  flush_batch: number
  log_level: string
  minimize_to_tray: boolean
  telemetry_enabled: boolean
}

// 后端事件载荷（与 mqttkit_ipc::event::AppEvent 各变体对应）
export interface StateChanged {
  connection_id: string
  state: ConnectionState
  detail?: string | null
}
export interface MessageBatch {
  connection_id: string
  messages: StoredMessage[]
}
export interface StatsEvent {
  connection_id: string
  stats: TrafficStats
}
export interface BackpressureEvent {
  connection_id: string
  degraded: boolean
}
