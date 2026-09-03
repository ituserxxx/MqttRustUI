import { defineStore } from 'pinia'
import { api } from '../api/backend'
import type { StoredMessage } from '../types'

// UI 侧内存上限（与 Rust 端 message_buffer 解耦，避免前端无限增长）
const UI_CAP = 50_000

export const useMessagesStore = defineStore('messages', {
  state: () => ({
    byConn: {} as Record<string, StoredMessage[]>,
    degraded: {} as Record<string, boolean>,
    stats: {} as Record<string, { received: number; sent: number; recv_rate: number }>,
    total: {} as Record<string, number>,
  }),
  actions: {
    async snapshot(connectionId: string, limit: number) {
      const msgs = await api.getMessages(connectionId, limit)
      this.byConn[connectionId] = msgs
    },
    ingestBatch(connectionId: string, msgs: StoredMessage[]) {
      const arr = this.byConn[connectionId] ?? (this.byConn[connectionId] = [])
      for (const m of msgs) arr.push(m)
      if (arr.length > UI_CAP) arr.splice(0, arr.length - UI_CAP)
      this.total[connectionId] = (this.total[connectionId] ?? 0) + msgs.length
    },
    setDegraded(connectionId: string, v: boolean) {
      this.degraded[connectionId] = v
    },
    setStats(connectionId: string, s: { received: number; sent: number; recv_rate: number }) {
      this.stats[connectionId] = s
    },
    async clear(connectionId: string) {
      await api.clearMessages(connectionId)
      this.byConn[connectionId] = []
    },
  },
})
