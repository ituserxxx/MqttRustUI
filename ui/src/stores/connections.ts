import { defineStore } from 'pinia'
import { api } from '../api/backend'
import type { Connection, ConnectionState } from '../types'

export const useConnectionsStore = defineStore('connections', {
  state: () => ({
    list: [] as Connection[],
    states: {} as Record<string, ConnectionState>,
    loading: false,
  }),
  getters: {
    active: (s) => (id: string) => s.list.find((c) => c.id === id),
  },
  actions: {
    async load() {
      this.loading = true
      try {
        this.list = await api.listConnections()
      } finally {
        this.loading = false
      }
    },
    async save(conn: Connection, password?: string) {
      await api.saveConnection(conn, password)
      await this.load()
    },
    async remove(id: string) {
      await api.deleteConnection(id)
      await this.load()
    },
    async connect(id: string) {
      await api.connect(id)
    },
    async disconnect(id: string) {
      await api.disconnect(id)
    },
    setState(id: string, st: ConnectionState) {
      this.states[id] = st
    },
  },
})
