<template>
  <div class="topic-tree">
    <div class="head">Topics</div>
    <a-tree
      v-if="treeData.length"
      :tree-data="treeData"
      :default-expand-all="true"
      block-node
      @select="onSelect"
    />
    <a-empty v-else description="暂无 topic" :image="simpleImage" />
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { Empty } from 'ant-design-vue'
import { useConnectionsStore } from '../stores/connections'
import { useMessagesStore } from '../stores/messages'

const props = defineProps<{ connectionId: string }>()
const connStore = useConnectionsStore()
const msgStore = useMessagesStore()
const simpleImage = Empty.PRESENTED_IMAGE_SIMPLE

interface Node {
  title: string
  key: string
  children: Record<string, Node>
  count: number
}

function ensure(root: Node, seg: string): Node {
  if (!root.children[seg]) {
    root.children[seg] = { title: seg, key: '', children: {}, count: 0 }
  }
  return root.children[seg]
}

function toAnt(node: Node, prefix: string): any {
  const key = prefix ? `${prefix}/${node.title}` : node.title
  node.key = key
  const children = Object.values(node.children).map((c) => toAnt(c, key))
  return {
    title: `${node.title} (${node.count})`,
    key,
    children: children.length ? children : undefined,
  }
}

const treeData = computed(() => {
  const root: Node = { title: '', key: '', children: {}, count: 0 }
  // 订阅的 topic 先入树
  const conn = connStore.list.find((c) => c.id === props.connectionId)
  for (const sub of conn?.subscriptions ?? []) {
    let cur = root
    for (const seg of sub.filter.split('/')) {
      cur = ensure(cur, seg)
    }
    cur.count = Math.max(cur.count, 0)
  }
  // 收到的消息 topic
  const msgs = msgStore.byConn[props.connectionId] ?? []
  const counts: Record<string, number> = {}
  for (const m of msgs) counts[m.topic] = (counts[m.topic] ?? 0) + 1
  for (const topic of Object.keys(counts)) {
    let cur = root
    for (const seg of topic.split('/')) cur = ensure(cur, seg)
    cur.count = counts[topic]
  }
  return Object.values(root.children).map((c) => toAnt(c, ''))
})

const emit = defineEmits<{ (e: 'select-topic', topic: string): void }>()

function onSelect(keys: any[]) {
  // 选中节点（key = 完整 topic 路径）时通知父级过滤消息列表
  const k = keys?.[0]
  if (typeof k === 'string' && k) emit('select-topic', k)
}
</script>

<style scoped>
.topic-tree {
  height: 100%;
  overflow: auto;
}
.head {
  padding: 8px 12px;
  font-weight: 600;
  border-bottom: 1px solid #f0f0f0;
}
</style>
