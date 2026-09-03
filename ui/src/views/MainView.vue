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
      </a-layout-header>

      <a-alert
        v-if="degradedBanner"
        type="warning"
        banner
        message="消息速率过快，已暂停明细推送，仅显示统计。积压回落后自动恢复。"
      />

      <a-layout-content class="content">
        <div class="panes">
          <TopicTree :connection-id="activeId" class="pane-topic" />
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
  </a-layout>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import ConnectionList from '../components/ConnectionList.vue'
import TopicTree from '../components/TopicTree.vue'
import MessageList from '../components/MessageList.vue'
import PublishPanel from '../components/PublishPanel.vue'
import { listenEvents } from '../api/backend'
import { useConnectionsStore } from '../stores/connections'
import { useMessagesStore } from '../stores/messages'
import { useSettingsStore } from '../stores/settings'
import type { ConnectionState, StoredMessage } from '../types'

const connStore = useConnectionsStore()
const msgStore = useMessagesStore()
const settingsStore = useSettingsStore()

const activeId = ref<string>('')
const search = ref('')
const degradedBanner = ref(false)

let unlisten: (() => void) | null = null

const stateColor = computed(() => {
  const s = activeId.value ? connStore.states[activeId.value] : undefined
  return (
    {
      idle: 'default',
      connecting: 'gold',
      connected: 'green',
      reconnecting: 'orange',
      failed: 'red',
      disconnecting: 'default',
    }[s ?? 'idle'] ?? 'default'
  )
})

const stateText = computed(() => {
  const s = (activeId.value ? connStore.states[activeId.value] : undefined) as
    | ConnectionState
    | undefined
  return (
    {
      idle: '未连接',
      connecting: '连接中',
      connected: '已连接',
      reconnecting: '重连中',
      failed: '失败',
      disconnecting: '断开中',
    }[s ?? 'idle'] ?? '未连接'
  )
})

const activeStats = computed(() =>
  activeId.value ? msgStore.stats[activeId.value] : undefined,
)

const filteredMessages = computed<StoredMessage[]>(() => {
  const list = activeId.value ? msgStore.byConn[activeId.value] ?? [] : []
  const q = search.value.trim().toLowerCase()
  if (!q) return list
  return list.filter(
    (m) =>
      m.topic.toLowerCase().includes(q) ||
      (m.preview ?? '').toLowerCase().includes(q),
  )
})

function onClear() {
  if (activeId.value) msgStore.clear(activeId.value)
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
})

onBeforeUnmount(() => unlisten?.())
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
