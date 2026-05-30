<template>
    <div class="dashboard">
        <div class="dashboard-hero">
            <div>
                <span class="hero-kicker">Financial Command Center</span>
                <h3>经营数据总览</h3>
                <p>集中查看收入、应收、结算和账本风险变化。</p>
            </div>
        </div>

        <!-- Stat Cards -->
        <div class="stat-cards">
            <div class="stat-card income">
                <div class="stat-icon">
                    <el-icon><Money /></el-icon>
                </div>
                <div class="stat-info">
                    <div class="stat-value">
                        ¥{{
                            (stats.current_month_income / 100).toLocaleString(
                                "zh-CN",
                                { minimumFractionDigits: 2 },
                            )
                        }}
                    </div>
                    <div class="stat-label">本月收入</div>
                </div>
            </div>

            <div class="stat-card unsettled">
                <div class="stat-icon">
                    <el-icon><Warning /></el-icon>
                </div>
                <div class="stat-info">
                    <div class="stat-value">
                        ¥{{
                            (stats.total_unsettled / 100).toLocaleString(
                                "zh-CN",
                                { minimumFractionDigits: 2 },
                            )
                        }}
                    </div>
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
      <el-radio-group v-model="rangeMonths" class="range-switch">
        <el-radio-button :label="6">近 6 个月</el-radio-button>
        <el-radio-button :label="12">近 12 个月</el-radio-button>
      </el-radio-group>
        </div>

        <!-- Charts -->
        <div class="chart-grid">
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

            <div class="chart-card">
                <h3>各账本未结清金额排名</h3>
                <div v-if="stats.book_ranking.length === 0" class="chart-empty">
                    <el-empty description="暂无数据" />
                </div>
                <v-chart
                    v-else
                    :option="chartOption"
                    style="height: 320px"
                    autoresize
                />
            </div>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from "vue";
import VChart from "vue-echarts";
import { Money, Warning, Document, Clock } from "@element-plus/icons-vue";
import { use } from "echarts/core";
import { BarChart, LineChart, PieChart } from "echarts/charts";
import {
    GridComponent,
    TooltipComponent,
    LegendComponent,
} from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";
import type { DashboardStats } from "@/types";
import { safeInvoke } from "@/utils/invoke";
import { useThemeStore } from "@/stores/theme";
import { getChartColors, getChartTextColor } from "@/utils/chart-theme";

use([
    BarChart,
    LineChart,
    PieChart,
    GridComponent,
    TooltipComponent,
    LegendComponent,
    CanvasRenderer,
]);

const stats = ref<DashboardStats>({
    current_month_income: 0,
    total_unsettled: 0,
    total_records: 0,
    pending_settlement: 0,
    book_ranking: [],
    income_trend: [],
    settlement_trend: [],
});

const rangeMonths = ref<6 | 12>(12);
const themeStore = useThemeStore();

// Backend now returns exactly rangeMonths of data — no client-side slice needed.
const incomeTrendData = computed(() => stats.value.income_trend);
const settlementTrendData = computed(() => stats.value.settlement_trend);
const incomeTrendHasData = computed(() =>
    incomeTrendData.value.some((item) => item.total_amount > 0),
);
const settlementTrendHasData = computed(() =>
    settlementTrendData.value.some(
        (item) => item.settled_amount > 0 || item.unsettled_amount > 0,
    ),
);
const incomeTrendOption = computed(() => {
    const colors = getChartColors();
    return {
        tooltip: {
            trigger: "axis",
            formatter: (params: any) => {
                if (!Array.isArray(params) || !params[0]) return "";
                const item = params[0];
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
            axisLabel: { color: getChartTextColor() },
        },
        yAxis: {
            type: "value",
            axisLabel: {
                formatter: (v: number) => `¥${(v / 100 / 10000).toFixed(1)}万`,
                color: getChartTextColor(),
            },
            splitLine: { lineStyle: { color: "var(--border-color)" } },
        },
        series: [
            {
                type: "line",
                data: incomeTrendData.value.map((item) => item.total_amount),
                smooth: true,
                showSymbol: false,
                lineStyle: { color: colors[0], width: 3 },
                areaStyle: { color: colors[0] + "40" },
            },
        ],
    };
});

const settlementTrendOption = computed(() => {
    const colors = getChartColors();
    return {
        tooltip: {
            trigger: "axis",
            axisPointer: { type: "shadow" },
            formatter: (params: any[]) => {
                if (!params?.length) return "";
                const sorted = [...params].sort((a, b) => b.value - a.value);
                const lines = sorted.map(
                    (item) =>
                        `${item.seriesName}: ¥${(item.value / 100).toLocaleString("zh-CN", { minimumFractionDigits: 2 })}`,
                );
                return `${sorted[0]?.axisValue || ""}<br/>${lines.join("<br/>")}`;
            },
        },
        legend: {
            top: 0,
            data: ["已收", "应收"],
            textStyle: { color: getChartTextColor() },
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
            axisLabel: { color: getChartTextColor() },
        },
        yAxis: {
            type: "value",
            axisLabel: {
                formatter: (v: number) => `¥${(v / 100 / 10000).toFixed(1)}万`,
                color: getChartTextColor(),
            },
            splitLine: { lineStyle: { color: "var(--border-color)" } },
        },
        series: [
            {
                name: "已收",
                type: "bar",
                stack: "total",
                data: settlementTrendData.value.map(
                    (item) => item.settled_amount,
                ),
                itemStyle: { color: colors[1], borderRadius: [4, 4, 0, 0] },
                barMaxWidth: 30,
            },
            {
                name: "应收",
                type: "bar",
                stack: "total",
                data: settlementTrendData.value.map(
                    (item) => item.unsettled_amount,
                ),
                itemStyle: { color: colors[2], borderRadius: [4, 4, 0, 0] },
                barMaxWidth: 30,
            },
        ],
    };
});

const chartOption = computed(() => {
    const colors = getChartColors();
    return {
        tooltip: {
            trigger: "axis",
            axisPointer: { type: "shadow" },
            formatter: (p: any) => {
                if (!Array.isArray(p) || !p[0]) return "";
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
                color: getChartTextColor(),
            },
            splitLine: { lineStyle: { color: "var(--border-color)" } },
        },
        yAxis: {
            type: "category",
            data: stats.value.book_ranking.map((b) => b.book_name),
            inverse: true,
            axisLabel: { color: getChartTextColor() },
        },
        series: [
            {
                type: "bar",
                data: stats.value.book_ranking.map((b, idx) => ({
                    value: b.unsettled_amount,
                    itemStyle: {
                        color: colors[idx % colors.length],
                        borderRadius: [0, 4, 4, 0],
                    },
                })),
                barMaxWidth: 28,
            },
        ],
    };
});

async function fetchStats() {
    try {
        stats.value = await safeInvoke<DashboardStats>("get_dashboard_stats", {
            rangeMonths: rangeMonths.value,
        });
    } catch {
        // Keep default zero values
    }
}

onMounted(() => {
    fetchStats();
});

watch(rangeMonths, () => {
    fetchStats();
});
</script>

<style scoped lang="scss">
.dashboard {
    display: flex;
    flex-direction: column;
    gap: 18px;
}

.dashboard-hero {
    display: flex;
    align-items: center;
    justify-content: space-between;
    min-height: 132px;
    padding: 26px 28px;
    overflow: hidden;
    border: 1px solid var(--border-color);
    border-radius: 14px;
    background:
        radial-gradient(
            circle at 78% 20%,
            rgba(37, 99, 235, 0.16),
            transparent 32%
        ),
        linear-gradient(135deg, var(--card-bg), var(--card-bg-subtle));
    box-shadow: var(--card-shadow);

    .hero-kicker {
        display: inline-block;
        margin-bottom: 10px;
        color: var(--color-primary);
        font-size: 13px;
        font-weight: 800;
        text-transform: uppercase;
    }

    h3 {
        margin-bottom: 8px;
        color: var(--text-heading);
        font-size: 24px;
        line-height: 1.25;
    }

    p {
        color: var(--text-secondary);
        font-size: 14px;
    }
}

.hero-status {
    min-width: 132px;
    padding: 16px;
    border: 1px solid var(--border-color);
    border-radius: 12px;
    background: var(--bg-elevated);
    text-align: right;

    span,
    strong {
        display: block;
    }

    span {
        color: var(--text-tertiary);
        font-size: 13px;
        margin-bottom: 6px;
    }

    strong {
        color: var(--text-heading);
        font-size: 18px;
    }
}

.stat-cards {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 16px;
}

.stat-card {
    background: var(--card-bg);
    border-radius: 14px;
    padding: 22px;
    display: flex;
    align-items: center;
    gap: 16px;
    border: 1px solid var(--border-color);
    transition:
        box-shadow 180ms ease,
        transform 180ms ease,
        border-color 180ms ease;

    &:hover {
        border-color: var(--color-primary);
        box-shadow: var(--card-shadow-hover);
        transform: translateY(-2px);
    }

    .stat-icon {
        width: 48px;
        height: 48px;
        border-radius: 12px;
        display: flex;
        align-items: center;
        justify-content: center;
        font-size: 24px;
        color: #fff;
    }

    &.income .stat-icon {
        color: var(--color-primary);
        background: var(--color-primary-soft);
    }
    &.unsettled .stat-icon {
        color: var(--color-danger);
        background: var(--color-danger-soft);
    }
    &.count .stat-icon {
        color: var(--color-success);
        background: var(--color-success-soft);
    }
    &.pending .stat-icon {
        color: var(--color-warning);
        background: var(--color-warning-soft);
    }

    .stat-info {
        .stat-value {
            font-size: 22px;
            font-weight: 700;
            color: var(--text-heading);
            margin-bottom: 4px;
        }
        .stat-label {
            font-size: 14px;
            color: var(--text-tertiary);
        }
    }
}

.chart-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 4px 2px;

    h3 {
        color: var(--text-heading);
        font-size: 18px;
  }
}

.range-switch {
  display: inline-flex;
  gap: 8px;
  padding: 4px;
  border: 1px solid transparent;
  border-radius: 12px;
  background: transparent;
  box-shadow: none;

  :deep(.el-radio-button__inner) {
    min-width: 118px;
    height: 40px;
    padding: 0 20px;
    color: var(--text-secondary);
    font-size: 15px;
    font-weight: 700;
    line-height: 40px;
    border: 0;
    border-radius: 8px;
    background: transparent;
    box-shadow: none;
    transition: background-color 180ms ease, color 180ms ease, box-shadow 180ms ease;
  }

  :deep(.el-radio-button:first-child .el-radio-button__inner),
  :deep(.el-radio-button:last-child .el-radio-button__inner) {
    border-radius: 8px;
  }

  :deep(.el-radio-button__original-radio:checked + .el-radio-button__inner) {
    color: #ffffff;
    background: var(--color-primary);
    box-shadow: 0 10px 24px rgba(37, 99, 235, 0.24);
  }
}

.chart-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 16px;
}

.chart-card {
    background: var(--card-bg);
    border-radius: 14px;
    padding: 24px;
    border: 1px solid var(--border-color);
    box-shadow: var(--card-shadow);
    transition:
        box-shadow 180ms ease,
        border-color 180ms ease,
        transform 180ms ease;

    &:hover {
        border-color: var(--border-hover);
        box-shadow: var(--card-shadow-hover);
        transform: translateY(-1px);
    }

    h3 {
        color: var(--text-heading);
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

@media (max-width: 1200px) {
    .stat-cards {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .chart-grid {
        grid-template-columns: 1fr;
    }
}
</style>
