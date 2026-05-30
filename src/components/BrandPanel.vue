<template>
  <section class="brand-panel" aria-label="FinLedger enterprise summary">
    <div class="brand-mark">
      <img :src="logoUrl" alt="" class="brand-logo" />
      <div>
        <span class="eyebrow">Enterprise Finance OS</span>
        <h1>FinLedger</h1>
      </div>
    </div>

    <div class="brand-copy">
      <p class="headline">让广告业务的每一笔收支，都进入清晰、安全、可追踪的财务中枢。</p>
      <p class="description">
        面向团队协作、客户项目、数据分析与备份治理的企业级记账平台。
      </p>
    </div>

    <div class="metric-board">
      <div class="metric-card">
        <el-icon><TrendCharts /></el-icon>
        <span>本月增长</span>
        <strong>28.6%</strong>
      </div>
      <div class="metric-card">
        <el-icon><DataAnalysis /></el-icon>
        <span>账本状态</span>
        <strong>实时同步</strong>
      </div>
      <div class="metric-card">
        <el-icon><Connection /></el-icon>
        <span>安全会话</span>
        <strong>已加密</strong>
      </div>
    </div>

    <div class="flow-line">
      <span />
      <span />
      <span />
    </div>
  </section>
</template>

<script setup lang="ts">
import {
  Connection,
  DataAnalysis,
  TrendCharts,
} from "@element-plus/icons-vue";
import logoUrl from "@/assets/finledger-logo.png";
</script>

<style scoped lang="scss">
.brand-panel {
  position: relative;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  padding: 52px;
  overflow: hidden;
  background: var(--bp-brand-bg);

  &::after {
    position: absolute;
    right: -120px;
    bottom: -150px;
    width: 390px;
    height: 390px;
    content: "";
    border: 1px solid rgba(125, 211, 252, 0.24);
    border-radius: 50%;
    box-shadow: inset 0 0 60px rgba(59, 130, 246, 0.14);
  }
}

.brand-mark {
  position: relative;
  z-index: 1;
  display: flex;
  gap: 18px;
  align-items: center;
}

.brand-logo {
  width: 72px;
  height: 72px;
  border: 1px solid rgba(255, 255, 255, 0.28);
  border-radius: 18px;
  object-fit: cover;
  box-shadow: 0 16px 40px rgba(14, 165, 233, 0.28);
}

.eyebrow {
  display: block;
  margin-bottom: 8px;
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0;
  color: var(--bp-brand-accent);
  text-transform: uppercase;
}

h1 {
  margin: 0;
  font-size: 34px;
  line-height: 1;
  color: var(--bp-brand-title);
}

.brand-copy {
  position: relative;
  z-index: 1;
  max-width: 570px;
  margin-top: 70px;
}

.headline {
  margin-bottom: 18px;
  font-size: 38px;
  font-weight: 750;
  line-height: 1.22;
  color: var(--bp-brand-text);
}

.description {
  max-width: 520px;
  font-size: 16px;
  line-height: 1.8;
  color: var(--bp-brand-muted);
}

.metric-board {
  position: relative;
  z-index: 1;
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 14px;
  margin-top: 50px;
}

.metric-card {
  min-height: 116px;
  padding: 18px;
  border: 1px solid var(--bp-brand-chip-border);
  border-radius: 14px;
  background: var(--bp-brand-chip-bg);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.08);
  animation: metric-rise 680ms cubic-bezier(0.22, 1, 0.36, 1) both;

  &:nth-child(2) {
    animation-delay: 90ms;
  }

  &:nth-child(3) {
    animation-delay: 180ms;
  }

  .el-icon {
    width: 34px;
    height: 34px;
    margin-bottom: 14px;
    color: var(--bp-brand-accent);
    border-radius: 10px;
    background: rgba(8, 145, 178, 0.18);
  }

  span,
  strong {
    display: block;
  }

  span {
    margin-bottom: 7px;
    font-size: 12px;
    color: var(--bp-brand-muted);
  }

  strong {
    font-size: 18px;
    color: var(--bp-brand-chip-text);
  }
}

.flow-line {
  position: absolute;
  right: 46px;
  bottom: 46px;
  left: 52px;
  height: 1px;
  overflow: hidden;
  background: rgba(148, 163, 184, 0.18);

  span {
    position: absolute;
    top: 0;
    width: 90px;
    height: 1px;
    background: linear-gradient(90deg, transparent, #67e8f9, transparent);
    animation: line-scan 3.4s linear infinite;
  }

  span:nth-child(2) {
    animation-delay: 1.1s;
  }

  span:nth-child(3) {
    animation-delay: 2.2s;
  }
}

@keyframes metric-rise {
  from {
    opacity: 0;
    transform: translateY(16px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}

@keyframes line-scan {
  from {
    transform: translateX(-120px);
  }
  to {
    transform: translateX(740px);
  }
}

@media (max-width: 1024px) {
  .brand-panel {
    padding: 34px;
  }

  .brand-copy {
    margin-top: 36px;
  }

  .headline {
    font-size: 28px;
  }

  .metric-board,
  .flow-line {
    display: none;
  }
}

@media (max-width: 560px) {
  .brand-panel {
    padding: 28px 24px;
  }

  .brand-logo {
    width: 58px;
    height: 58px;
    border-radius: 14px;
  }

  h1 {
    font-size: 28px;
  }

  .headline {
    font-size: 24px;
  }

  .description {
    font-size: 14px;
  }
}
</style>
