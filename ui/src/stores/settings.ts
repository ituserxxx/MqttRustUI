import { defineStore } from 'pinia'
import { api } from '../api/backend'
import type { AppSettings } from '../types'

export const useSettingsStore = defineStore('settings', {
  state: () => ({
    settings: {
      theme: 'auto',
      locale: 'zh-CN',
      message_buffer: 100000,
      flush_interval_ms: 50,
      flush_batch: 200,
      log_level: 'info',
      minimize_to_tray: true,
      telemetry_enabled: false,
    } as AppSettings,
  }),
  actions: {
    async load() {
      this.settings = await api.getSettings()
    },
    async save() {
      await api.saveSettings(this.settings)
    },
  },
})
