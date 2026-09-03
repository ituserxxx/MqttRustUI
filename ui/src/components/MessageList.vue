<template>
  <div class="msg-wrap" ref="wrap" @scroll="onScroll">
    <div class="spacer" :style="{ height: totalHeight + 'px' }">
      <div class="rows" :style="{ transform: `translateY(${offset}px)` }">
        <div v-for="m in visible" :key="m.id" class="msg-row">
          <span class="time">{{ fmt(m.timestamp) }}</span>
          <span class="topic" :title="m.topic">{{ m.topic }}</span>
          <a-tag :color="qosColor(m.qos)">{{ qosText(m.qos) }}</a-tag>
          <a-tag v-if="m.retain" color="purple">R</a-tag>
          <span class="preview">{{ m.preview || '(二进制 ' + m.size + 'B)' }}</span>
        </div>
      </div>
    </div>
    <a-empty v-if="!messages.length" description="暂无消息" :image="simpleImage" class="empty" />
  </div>
</template>

<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue'
import { Empty } from 'ant-design-vue'
import type { StoredMessage, Qos } from '../types'

const props = defineProps<{ messages: StoredMessage[]; connectionId: string }>()
const simpleImage = Empty.PRESENTED_IMAGE_SIMPLE

const ROW = 30
const OVERSCAN = 10
const wrap = ref<HTMLElement | null>(null)
const scrollTop = ref(0)
const viewH = ref(600)

const totalHeight = computed(() => props.messages.length * ROW)
const start = computed(() => Math.max(0, Math.floor(scrollTop.value / ROW) - OVERSCAN))
const visibleCount = computed(() =>
  Math.ceil(viewH.value / ROW) + OVERSCAN * 2,
)
const visible = computed(() =>
  props.messages.slice(start.value, start.value + visibleCount.value),
)
const offset = computed(() => start.value * ROW)

function onScroll(e: Event) {
  scrollTop.value = (e.target as HTMLElement).scrollTop
}

function measure() {
  if (wrap.value) viewH.value = wrap.value.clientHeight
}
onMounted(() => {
  measure()
  window.addEventListener('resize', measure)
})
onBeforeUnmount(() => window.removeEventListener('resize', measure))

function fmt(ts: number): string {
  const d = new Date(ts)
  const p = (n: number) => String(n).padStart(2, '0')
  return `${p(d.getHours())}:${p(d.getMinutes())}:${p(d.getSeconds())}`
}
function qosColor(q: Qos) {
  return q === 'exactly_once' ? 'red' : q === 'at_least_once' ? 'blue' : 'default'
}
function qosText(q: Qos) {
  return q === 'exactly_once' ? 'Q2' : q === 'at_least_once' ? 'Q1' : 'Q0'
}
</script>

<style scoped>
.msg-wrap {
  height: 100%;
  overflow-y: auto;
  position: relative;
  background: #fafafa;
}
.spacer {
  position: relative;
}
.rows {
  position: absolute;
  top: 0;
  left: 0;
  right: 0;
}
.msg-row {
  height: 30px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 0 12px;
  border-bottom: 1px solid #f0f0f0;
  font-size: 12px;
  white-space: nowrap;
}
.time {
  color: #999;
  width: 64px;
  flex: none;
}
.topic {
  color: #1677ff;
  max-width: 280px;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: none;
}
.preview {
  color: #333;
  overflow: hidden;
  text-overflow: ellipsis;
  flex: 1;
}
.empty {
  margin-top: 80px;
}
</style>
