<template>
  <a-layout class="root">
    <a-layout-sider width="240" theme="light" class="sider">
      <ConnectionList v-model:active-id="activeId" />
    </a-layout-sider>

    <a-layout>
      <a-layout-header class="toolbar">
        <span class="title">MqttRustUI</span>
        <a-tag :color="stateColor">{{ stateText }}</a-tag>
        <span class="stat" v-if="activeStats">
          收 {{ activeStats.received }} · 速率 {{ activeStats.recv_rate.toFixed(0) }}/s
        </span>
        <a-input-search
          v-model:value="search"
          placeholder="按 topic / payload 过滤"
          class="search"
          allow-clear
        />
        <a-button @click="onClear" :disabled="!activeId">清屏</a-button>
        <a-button @click="settingsOpen = true">设置</a-button>
      </a-layout-header>

      <a-alert
        v-if="degradedBanner"
        type="warning"
        banner
        message="消息速率过快，已暂停明细推送，仅显示统计。积压回落后自动恢复。"
      />

      <a-layout-content class="content">
        <div class="panes">
          <TopicTree :connection-id="activeId" class="pane-topic" @select-topic="onSelectTopic" />
          <MessageList
            :messages="filteredMessages"
            :connection-id="activeId"
            class="pane-msg"
          />
        </div>
      </a-layout-content>

      <a-layout-footer class="footer">
        <PublishPanel :connection-id="activeId" />
      </a-layout-footer>
    </a-layout>

    <SettingsModal v-model:open="settingsOpen" />
  </a-layout>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { message } from 'ant-design-vue'
import { listen } from '@tauri-apps/api/event'
import { save, open as openDialog } from '@tauri-apps/plugin-dialog'
import ConnectionList from '../components/ConnectionList.vue'
import TopicTree from '../components/TopicTree.vue'
import MessageList from '../components/MessageList.vue'
import PublishPanel from '../components/PublishPanel.vue'
import SettingsModal from '../components/SettingsModal.vue'
import { listenEvents, api } from '../api/backend'
import { useConnectionsStore } from '../stores/connections'
import { useMessagesStore } from '../stores/messages'
import { useSettingsStore } from '../stores/settings'
import type { ConnectionState, StoredMessage } from '../types'

const connStore = useConnectionsStore()
const msgStore = useMessagesStore()
const settingsStore = useSettingsStore()

const activeId = ref<string>('')
const search = ref('')
const topicFilter = ref('')
const degradedBanner = ref(false)
const settingsOpen = ref(false)

let unlisten: (() => void) | null = null
let unlistenMenu: (() => void) | null = null

const stateColor = computed(() => {
  const s = (activeId.value ? connStore.states[activeId.value] : undefined) ?? 'idle'
  const map: Record<ConnectionState, string> = {
    idle: 'default',
    connecting: 'gold',
    connected: 'green',
    reconnecting: 'orange',
    failed: 'red',
    disconnecting: 'default',
  }
  return map[s]
})

const stateText = computed(() => {
  const s = (activeId.value ? connStore.states[activeId.value] : undefined) ?? 'idle'
  const map: Record<ConnectionState, string> = {
    idle: '未连接',
    connecting: '连接中',
    connected: '已连接',
    reconnecting: '重连中',
    failed: '失败',
    disconnecting: '断开中',
  }
  return map[s]
})

const activeStats = computed(() =>
  activeId.value ? msgStore.stats[activeId.value] : undefined,
)

const filteredMessages = computed<StoredMessage[]>(() => {
  const list = activeId.value ? msgStore.byConn[activeId.value] ?? [] : []
  let out = list
  // Topic 树选中的精确过滤（点击树节点后只看该 topic）
  if (topicFilter.value) {
    out = out.filter((m) => m.topic.startsWith(topicFilter.value))
  }
  const q = search.value.trim().toLowerCase()
  if (!q) return out
  return out.filter(
    (m) =>
      m.topic.toLowerCase().includes(q) ||
      (m.preview ?? '').toLowerCase().includes(q),
  )
})

function onClear() {
  if (activeId.value) msgStore.clear(activeId.value)
}

function onSelectTopic(topic: string) {
  // 再点一次同一节点取消过滤
  topicFilter.value = topicFilter.value === topic ? '' : topic
}

// 菜单事件：导入/导出配置、打开设置
async function onExport() {
  try {
    const json = await api.exportConfig()
    const path = await save({ defaultPath: 'mqttkit-config.json', filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!path) return
    const { writeTextFile } = await import('@tauri-apps/plugin-fs')
    await writeTextFile(path, json)
    message.success('配置已导出到 ' + path)
  } catch (e) {
    message.error('导出失败：' + String(e))
  }
}
async function onImport() {
  try {
    const path = await openDialog({ filters: [{ name: 'JSON', extensions: ['json'] }] })
    if (!path || Array.isArray(path)) return
    const { readTextFile } = await import('@tauri-apps/plugin-fs')
    const json = await readTextFile(path)
    await api.importConfig(json)
    await connStore.load()
    message.success('配置已导入')
  } catch (e) {
    message.error('导入失败：' + String(e))
  }
}

// 切换连接时回填最近消息
watch(activeId, async (id) => {
  if (id) await msgStore.snapshot(id, 5000)
})

onMounted(async () => {
  await settingsStore.load()
  await connStore.load()
  if (connStore.list.length) activeId.value = connStore.list[0].id

  unlisten = await listenEvents({
    onState: (p) => {
      connStore.setState(p.connection_id, p.state)
    },
    onBatch: (p) => {
      if (p.connection_id === activeId.value) msgStore.ingestBatch(p.connection_id, p.messages)
    },
    onStats: (p) => {
      msgStore.setStats(p.connection_id, p.stats)
    },
    onBackpressure: (p) => {
      msgStore.setDegraded(p.connection_id, p.degraded)
      if (p.connection_id === activeId.value) degradedBanner.value = p.degraded
    },
  })

  // 菜单事件
  const u1 = await listen('menu:settings', () => { settingsOpen.value = true })
  const u2 = await listen('menu:import_cfg', onImport)
  const u3 = await listen('menu:export_cfg', onExport)
  unlistenMenu = () => { u1(); u2(); u3() }
})

onBeforeUnmount(() => {
  unlisten?.()
  unlistenMenu?.()
})
</script>

<style scoped>
.root {
  height: 100vh;
}
.sider {
  border-right: 1px solid #f0f0f0;
}
.toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  background: #fff;
  border-bottom: 1px solid #f0f0f0;
  padding: 0 16px;
}
.title {
  font-weight: 600;
}
.search {
  width: 280px;
  margin-left: auto;
}
.content {
  overflow: hidden;
  padding: 0;
}
.panes {
  display: flex;
  height: 100%;
}
.pane-topic {
  width: 260px;
  border-right: 1px solid #f0f0f0;
  overflow: auto;
}
.pane-msg {
  flex: 1;
  overflow: hidden;
}
.footer {
  background: #fff;
  border-top: 1px solid #f0f0f0;
  padding: 8px 16px;
}
</style>
