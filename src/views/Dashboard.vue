<template>
  <div class="dashboard">
    <h2>首页看板</h2>

    <!-- Stat Cards -->
    <div class="stat-cards">
      <div class="stat-card income">
        <div class="stat-icon">
          <el-icon><Money /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-value">¥{{ stats.current_month_income.toLocaleString("zh-CN", { minimumFractionDigits: 2 }) }}</div>
          <div class="stat-label">本月收入</div>
        </div>
      </div>

      <div class="stat-card unsettled">
        <div class="stat-icon">
          <el-icon><Warning /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-value">¥{{ stats.total_unsettled.toLocaleString("zh-CN", { minimumFractionDigits: 2 }) }}</div>
          <div class="stat-label">未结清总额</div>
        </div>
      </div>

      <div class="stat-card count">
        <div class="stat-icon">
          <el-icon><Document /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-value">{{ stats.total_records }}</div>
          <div class="stat-label">总记录数</div>
        </div>
      </div>

      <div class="stat-card pending">
        <div class="stat-icon">
          <el-icon><Clock /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-value">{{ stats.pending_settlement }}</div>
          <div class="stat-label">待结算</div>
        </div>
      </div>
    </div>

    <!-- Chart -->
    <div class="chart-section">
      <div class="chart-card">
        <h3>各账本未结清金额排名</h3>
        <div v-if="stats.book_ranking.length === 0" class="chart-empty">
          <el-empty description="暂无数据" />
        </div>
        <v-chart
          v-else
          :option="chartOption"
          style="height: 360px"
          autoresize
        />
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import VChart from "vue-echarts";
import { Money, Warning, Document, Clock } from "@element-plus/icons-vue";
import { use } from "echarts/core";
import { BarChart } from "echarts/charts";
import { GridComponent, TooltipComponent, TitleComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { DashboardStats } from "@/types";

use([BarChart, GridComponent, TooltipComponent, TitleComponent, CanvasRenderer]);

const stats = ref<DashboardStats>({
  current_month_income: 0,
  total_unsettled: 0,
  total_records: 0,
  pending_settlement: 0,
  book_ranking: [],
});

const chartOption = computed(() => ({
  tooltip: {
    trigger: "axis",
    axisPointer: { type: "shadow" },
    formatter: (p: any) => {
      const item = p[0];
      return `${item.name}<br/>未结清金额: ¥${item.value.toLocaleString("zh-CN", { minimumFractionDigits: 2 })}`;
    },
  },
  grid: {
    left: "3%",
    right: "10%",
    bottom: "3%",
    containLabel: true,
  },
  xAxis: {
    type: "value",
    axisLabel: {
      formatter: (v: number) => `¥${(v / 10000).toFixed(1)}万`,
    },
  },
  yAxis: {
    type: "category",
    data: stats.value.book_ranking.map((b) => b.book_name),
    inverse: true,
  },
  series: [
    {
      type: "bar",
      data: stats.value.book_ranking.map((b) => ({
        value: b.unsettled_amount,
        itemStyle: {
          color: (() => {
            const colors = [
              "#f56c6c", "#e6a23c", "#409eff", "#67c23a", "#909399",
              "#f56c6c", "#e6a23c", "#409eff", "#67c23a", "#909399",
            ];
            const idx = stats.value.book_ranking.indexOf(b);
            return colors[idx % colors.length];
          })(),
          borderRadius: [0, 4, 4, 0],
        },
      })),
      barMaxWidth: 28,
    },
  ],
}));

onMounted(async () => {
  try {
    stats.value = await invoke<DashboardStats>("get_dashboard_stats");
  } catch {
    // Keep default zero values
  }
});
</script>

<style scoped lang="scss">
.dashboard {
  h2 {
    font-size: 20px;
    margin-bottom: 24px;
  }
}

.stat-cards {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 20px;
  margin-bottom: 24px;
}

.stat-card {
  background: #fff;
  border-radius: 10px;
  padding: 24px;
  display: flex;
  align-items: center;
  gap: 16px;
  border: 1px solid #ebeef5;
  transition: box-shadow 180ms ease, transform 180ms ease, border-color 180ms ease;

  &:hover {
    border-color: #dcdfe6;
    box-shadow: 0 8px 22px rgba(31, 45, 61, 0.08);
    transform: translateY(-2px);
  }

  .stat-icon {
    width: 48px;
    height: 48px;
    border-radius: 10px;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 24px;
    color: #fff;
  }

  &.income .stat-icon { background: #409eff; }
  &.unsettled .stat-icon { background: #f56c6c; }
  &.count .stat-icon { background: #67c23a; }
  &.pending .stat-icon { background: #e6a23c; }

  .stat-info {
    .stat-value {
      font-size: 22px;
      font-weight: 700;
      color: #303133;
      margin-bottom: 4px;
    }
    .stat-label {
      font-size: 13px;
      color: #909399;
    }
  }
}

.chart-section {
  .chart-card {
    background: #fff;
    border-radius: 10px;
    padding: 24px;
    border: 1px solid #ebeef5;
    transition: box-shadow 180ms ease, border-color 180ms ease;

    &:hover {
      border-color: #dcdfe6;
      box-shadow: 0 8px 22px rgba(31, 45, 61, 0.06);
    }

    h3 {
      font-size: 16px;
      margin-bottom: 20px;
    }

    .chart-empty {
      height: 360px;
      display: flex;
      align-items: center;
      justify-content: center;
    }
  }
}
</style>
