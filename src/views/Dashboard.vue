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
          <div class="stat-value">¥{{ (stats.current_month_income / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 }) }}</div>
          <div class="stat-label">本月收入</div>
        </div>
      </div>

      <div class="stat-card unsettled">
        <div class="stat-icon">
          <el-icon><Warning /></el-icon>
        </div>
        <div class="stat-info">
          <div class="stat-value">¥{{ (stats.total_unsettled / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 }) }}</div>
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

    <!-- Range toggle -->
    <div class="chart-header">
      <h3>收入趋势</h3>
      <el-radio-group v-model="rangeMonths" size="small">
        <el-radio-button :label="6">近 6 个月</el-radio-button>
        <el-radio-button :label="12">近 12 个月</el-radio-button>
      </el-radio-group>
    </div>

    <!-- Income trend + Settlement stacked bar -->
    <div class="trend-grid">
      <div class="chart-card">
        <h3>近 {{ rangeMonths }} 个月收入趋势</h3>
        <div v-if="!incomeTrendHasData" class="chart-empty">
          <el-empty description="暂无数据" />
        </div>
        <v-chart
          v-else
          :option="incomeTrendOption"
          style="height: 320px"
          autoresize
        />
      </div>

      <div class="chart-card">
        <h3>已收 / 应收金额趋势</h3>
        <div v-if="!settlementTrendHasData" class="chart-empty">
          <el-empty description="暂无数据" />
        </div>
        <v-chart
          v-else
          :option="settlementTrendOption"
          style="height: 320px"
          autoresize
        />
      </div>
    </div>

    <!-- Category pie -->
    <div class="chart-card pie-card">
      <h3>近 12 个月收入类别占比</h3>
      <div v-if="!categoryShareHasData" class="chart-empty">
        <el-empty description="暂无数据" />
      </div>
      <v-chart
        v-else
        :option="categoryShareOption"
        style="height: 340px"
        autoresize
      />
    </div>

    <!-- Existing book ranking chart -->
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
import VChart from "vue-echarts";
import { Money, Warning, Document, Clock } from "@element-plus/icons-vue";
import { use } from "echarts/core";
import { BarChart, LineChart, PieChart } from "echarts/charts";
import { GridComponent, TooltipComponent, LegendComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { DashboardStats, IncomeCategory } from "@/types";
import { IncomeCategoryLabels } from "@/types";
import { safeInvoke } from "@/utils/invoke";

use([BarChart, LineChart, PieChart, GridComponent, TooltipComponent, LegendComponent, CanvasRenderer]);

const stats = ref<DashboardStats>({
  current_month_income: 0,
  total_unsettled: 0,
  total_records: 0,
  pending_settlement: 0,
  book_ranking: [],
  income_trend: [],
  settlement_trend: [],
  category_share: [],
});

const rangeMonths = ref<6 | 12>(12);

const incomeTrendData = computed(() => stats.value.income_trend.slice(-rangeMonths.value));
const settlementTrendData = computed(() => stats.value.settlement_trend.slice(-rangeMonths.value));
const categoryShareData = computed(() => stats.value.category_share);

const incomeTrendHasData = computed(() => incomeTrendData.value.some((item) => item.total_amount > 0));
const settlementTrendHasData = computed(() =>
  settlementTrendData.value.some((item) => item.settled_amount > 0 || item.unsettled_amount > 0),
);
const categoryShareHasData = computed(() => categoryShareData.value.some((item) => item.amount > 0));

const incomeTrendOption = computed(() => ({
  tooltip: {
    trigger: "axis",
    formatter: (params: any) => {
      const item = params?.[0];
      if (!item) return "";
      return `${item.axisValue}<br/>收入: ¥${(item.data / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 })}`;
    },
  },
  grid: {
    left: "3%",
    right: "6%",
    bottom: "3%",
    containLabel: true,
  },
  xAxis: {
    type: "category",
    data: incomeTrendData.value.map((item) => item.month),
  },
  yAxis: {
    type: "value",
    axisLabel: {
      formatter: (v: number) => `¥${(v / 100 / 10000).toFixed(1)}万`,
    },
  },
  series: [
    {
      type: "line",
      data: incomeTrendData.value.map((item) => item.total_amount),
      smooth: true,
      showSymbol: false,
      lineStyle: { color: "#409eff", width: 3 },
      areaStyle: { color: "rgba(64, 158, 255, 0.15)" },
    },
  ],
}));

const settlementTrendOption = computed(() => ({
  tooltip: {
    trigger: "axis",
    axisPointer: { type: "shadow" },
    formatter: (params: any[]) => {
      if (!params?.length) return "";
      const sorted = [...params].sort((a, b) => b.value - a.value);
      const lines = sorted.map(
        (item) => `${item.seriesName}: ¥${(item.value / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 })}`,
      );
      return `${sorted[0]?.axisValue || ""}<br/>${lines.join("<br/>")}`;
    },
  },
  legend: {
    top: 0,
    data: ["已收", "应收"],
  },
  grid: {
    left: "3%",
    right: "6%",
    bottom: "3%",
    containLabel: true,
  },
  xAxis: {
    type: "category",
    data: settlementTrendData.value.map((item) => item.month),
  },
  yAxis: {
    type: "value",
    axisLabel: {
      formatter: (v: number) => `¥${(v / 100 / 10000).toFixed(1)}万`,
    },
  },
  series: [
    {
      name: "已收",
      type: "bar",
      stack: "total",
      data: settlementTrendData.value.map((item) => item.settled_amount),
      itemStyle: { color: "#67c23a", borderRadius: [4, 4, 0, 0] },
      barMaxWidth: 30,
    },
    {
      name: "应收",
      type: "bar",
      stack: "total",
      data: settlementTrendData.value.map((item) => item.unsettled_amount),
      itemStyle: { color: "#f56c6c", borderRadius: [4, 4, 0, 0] },
      barMaxWidth: 30,
    },
  ],
}));

const categoryShareOption = computed(() => ({
  tooltip: {
    trigger: "item",
    formatter: (item: any) => {
      const name = item.name || "";
      return `${name}<br/>¥${(item.value / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 })} (${item.percent}%)`;
    },
  },
  legend: {
    bottom: 0,
    type: "scroll",
  },
  series: [
    {
      type: "pie",
      radius: ["45%", "65%"],
      center: ["50%", "45%"],
      data: categoryShareData.value.map((item) => ({
        name: IncomeCategoryLabels[item.category as IncomeCategory] || item.category,
        value: item.amount,
      })),
      label: { formatter: "{b}" },
    },
  ],
}));

const chartOption = computed(() => ({
  tooltip: {
    trigger: "axis",
    axisPointer: { type: "shadow" },
    formatter: (p: any) => {
      const item = p[0];
      return `${item.name}<br/>未结清金额: ¥${(item.value / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 })}`;
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
      formatter: (v: number) => `¥${(v / 100 / 10000).toFixed(1)}万`,
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
    stats.value = await safeInvoke<DashboardStats>("get_dashboard_stats");
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

.chart-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;

  h3 {
    font-size: 16px;
  }
}

.trend-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 20px;
  margin-bottom: 20px;
}

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
    height: 320px;
    display: flex;
    align-items: center;
    justify-content: center;
  }
}

.pie-card {
  margin-bottom: 20px;

  .chart-empty {
    height: 340px;
  }
}

.chart-section {
  margin-top: 20px;

  .chart-empty {
    height: 360px;
  }
}
</style>
