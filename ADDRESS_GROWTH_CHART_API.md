# Address Growth Chart API Documentation

## Overview
The Address Growth Chart API provides time-series data showing the growth of unique addresses on the Taiko blockchain over time. This is perfect for creating charts showing user adoption and network growth.

## GraphQL API

### Endpoint
```
POST /graphql
```

### Query
```graphql
query AddressGrowthChart($timeRange: TimeRange!) {
  addressGrowthChart(timeRange: $timeRange) {
    data {
      timestamp          # ISO string: "2024-05-27T00:00:00Z"
      totalAddresses     # Cumulative count up to this date
      newAddresses       # New addresses that appeared on this date
    }
    totalAddresses       # Current total unique addresses
    growthRate          # Percentage growth over the selected period
    timeRange           # Selected time range ("7D", "30D", "3M", "ALL")
    dataPoints          # Number of data points in the response
  }
}
```

### Time Range Options
- `LAST_7_DAYS` - Last 7 days with daily data points
- `LAST_30_DAYS` - Last 30 days with daily data points  
- `LAST_3_MONTHS` - Last 3 months with daily data points
- `ALL_TIME` - All available data with daily data points

## Frontend Integration Examples

### React with Recharts
```typescript
import { useQuery } from '@apollo/client';
import { LineChart, Line, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';

const ADDRESS_GROWTH_QUERY = gql`
  query AddressGrowthChart($timeRange: TimeRange!) {
    addressGrowthChart(timeRange: $timeRange) {
      data {
        timestamp
        totalAddresses
        newAddresses
      }
      totalAddresses
      growthRate
      timeRange
    }
  }
`;

function AddressGrowthChart({ timeRange = 'LAST_30_DAYS' }) {
  const { data, loading, error } = useQuery(ADDRESS_GROWTH_QUERY, {
    variables: { timeRange }
  });

  if (loading) return <div>Loading chart...</div>;
  if (error) return <div>Error: {error.message}</div>;

  const chartData = data.addressGrowthChart.data.map(point => ({
    date: new Date(point.timestamp).toLocaleDateString(),
    totalAddresses: point.totalAddresses,
    newAddresses: point.newAddresses,
  }));

  return (
    <div className="chart-container">
      <h3>Address Growth ({data.addressGrowthChart.timeRange})</h3>
      <p>Total: {data.addressGrowthChart.totalAddresses.toLocaleString()} addresses</p>
      <p>Growth: {data.addressGrowthChart.growthRate.toFixed(2)}%</p>
      
      <ResponsiveContainer width="100%" height={400}>
        <LineChart data={chartData}>
          <CartesianGrid strokeDasharray="3 3" />
          <XAxis dataKey="date" />
          <YAxis />
          <Tooltip />
          <Line 
            type="monotone" 
            dataKey="totalAddresses" 
            stroke="#8884d8" 
            strokeWidth={2}
            name="Total Addresses"
          />
        </LineChart>
      </ResponsiveContainer>
    </div>
  );
}
```

### React with Chart.js
```typescript
import { Line } from 'react-chartjs-2';
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  PointElement,
  LineElement,
  Title,
  Tooltip,
  Legend,
} from 'chart.js';

ChartJS.register(CategoryScale, LinearScale, PointElement, LineElement, Title, Tooltip, Legend);

function AddressGrowthChart({ timeRange = 'LAST_30_DAYS' }) {
  const { data } = useQuery(ADDRESS_GROWTH_QUERY, {
    variables: { timeRange }
  });

  if (!data) return null;

  const chartConfig = {
    labels: data.addressGrowthChart.data.map(point => 
      new Date(point.timestamp).toLocaleDateString()
    ),
    datasets: [
      {
        label: 'Total Addresses',
        data: data.addressGrowthChart.data.map(point => point.totalAddresses),
        borderColor: 'rgb(75, 192, 192)',
        backgroundColor: 'rgba(75, 192, 192, 0.2)',
        tension: 0.1,
      },
      {
        label: 'New Addresses (Daily)',
        data: data.addressGrowthChart.data.map(point => point.newAddresses),
        borderColor: 'rgb(255, 99, 132)',
        backgroundColor: 'rgba(255, 99, 132, 0.2)',
        tension: 0.1,
        yAxisID: 'y1',
      },
    ],
  };

  const options = {
    responsive: true,
    interaction: {
      mode: 'index' as const,
      intersect: false,
    },
    scales: {
      x: {
        display: true,
        title: {
          display: true,
          text: 'Date'
        }
      },
      y: {
        type: 'linear' as const,
        display: true,
        position: 'left' as const,
        title: {
          display: true,
          text: 'Total Addresses'
        }
      },
      y1: {
        type: 'linear' as const,
        display: true,
        position: 'right' as const,
        title: {
          display: true,
          text: 'New Addresses'
        },
        grid: {
          drawOnChartArea: false,
        },
      },
    },
  };

  return <Line data={chartConfig} options={options} />;
}
```

### Vue.js with Chart.js
```vue
<template>
  <div class="chart-container">
    <h3>Address Growth ({{ chartData?.timeRange }})</h3>
    <div class="stats">
      <span>Total: {{ chartData?.totalAddresses?.toLocaleString() }} addresses</span>
      <span>Growth: {{ chartData?.growthRate?.toFixed(2) }}%</span>
    </div>
    <canvas ref="chartCanvas"></canvas>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue';
import { Chart } from 'chart.js';
import { useQuery } from '@vue/apollo-composable';
import { ADDRESS_GROWTH_QUERY } from '@/queries/charts';

const props = defineProps<{
  timeRange: 'LAST_7_DAYS' | 'LAST_30_DAYS' | 'LAST_3_MONTHS' | 'ALL_TIME';
}>();

const chartCanvas = ref<HTMLCanvasElement>();
const chart = ref<Chart>();

const { result: chartData } = useQuery(ADDRESS_GROWTH_QUERY, () => ({
  timeRange: props.timeRange
}));

onMounted(() => {
  if (chartCanvas.value) {
    chart.value = new Chart(chartCanvas.value, {
      type: 'line',
      data: {
        labels: [],
        datasets: [{
          label: 'Total Addresses',
          data: [],
          borderColor: 'rgb(75, 192, 192)',
          tension: 0.1
        }]
      }
    });
  }
});

watch(chartData, (newData) => {
  if (newData && chart.value) {
    chart.value.data.labels = newData.addressGrowthChart.data.map(
      (point: any) => new Date(point.timestamp).toLocaleDateString()
    );
    chart.value.data.datasets[0].data = newData.addressGrowthChart.data.map(
      (point: any) => point.totalAddresses
    );
    chart.value.update();
  }
});
</script>
```

## Data Format

### Response Structure
```typescript
interface AddressGrowthChart {
  data: AddressChartPoint[];           // Time series data points
  totalAddresses: number;              // Current total
  growthRate: number;                  // Percentage growth
  timeRange: string;                   // "7D" | "30D" | "3M" | "ALL"
  dataPoints: number;                  // Number of points
}

interface AddressChartPoint {
  timestamp: string;                   // ISO string "2024-05-27T00:00:00Z"
  totalAddresses: number;              // Cumulative count
  newAddresses: number;                // New addresses this day
}
```

### Sample Response
```json
{
  "data": {
    "addressGrowthChart": {
      "data": [
        {
          "timestamp": "2024-05-25T00:00:00Z",
          "totalAddresses": 850,
          "newAddresses": 4
        },
        {
          "timestamp": "2024-05-27T00:00:00Z", 
          "totalAddresses": 1912,
          "newAddresses": 1062
        }
      ],
      "totalAddresses": 1912,
      "growthRate": 125.18,
      "timeRange": "30D",
      "dataPoints": 2
    }
  }
}
```

## Performance Notes

- **Caching**: Results are calculated in real-time but can be cached on the frontend for 5-10 minutes
- **Data Points**: Each time range returns daily aggregated data
- **Rate Limits**: No specific limits, but avoid excessive polling
- **Optimization**: Use appropriate time ranges - shorter ranges for detailed views, longer for trends

## Error Handling

```typescript
const { data, loading, error } = useQuery(ADDRESS_GROWTH_QUERY, {
  variables: { timeRange },
  errorPolicy: 'partial',
  onError: (error) => {
    console.error('Chart query failed:', error);
    // Show fallback UI or retry logic
  }
});

if (error) {
  return (
    <div className="error-state">
      <p>Failed to load chart data</p>
      <button onClick={() => refetch()}>Retry</button>
    </div>
  );
}
```

## Integration Tips

1. **Time Range Selector**: Provide buttons to switch between time ranges
2. **Loading States**: Show skeleton or spinner while loading
3. **Responsive Design**: Use ResponsiveContainer for charts
4. **Tooltips**: Format numbers with commas and meaningful labels
5. **Color Scheme**: Use consistent colors with your app theme
6. **Accessibility**: Add proper ARIA labels and keyboard navigation

## Advanced Usage

### Custom Time Range Component
```typescript
function TimeRangeSelector({ value, onChange }) {
  const ranges = [
    { key: 'LAST_7_DAYS', label: '7D' },
    { key: 'LAST_30_DAYS', label: '30D' },
    { key: 'LAST_3_MONTHS', label: '3M' },
    { key: 'ALL_TIME', label: 'ALL' },
  ];

  return (
    <div className="time-range-selector">
      {ranges.map(range => (
        <button
          key={range.key}
          className={value === range.key ? 'active' : ''}
          onClick={() => onChange(range.key)}
        >
          {range.label}
        </button>
      ))}
    </div>
  );
}
```

### Dashboard Card Component
```typescript
function AddressGrowthCard({ timeRange = 'LAST_30_DAYS' }) {
  const { data } = useQuery(ADDRESS_GROWTH_QUERY, {
    variables: { timeRange }
  });

  return (
    <div className="dashboard-card">
      <h3>Address Growth</h3>
      <div className="metric">
        <span className="value">
          {data?.addressGrowthChart.totalAddresses?.toLocaleString()}
        </span>
        <span className="label">Total Addresses</span>
      </div>
      <div className="growth-indicator">
        <span className={`rate ${data?.addressGrowthChart.growthRate >= 0 ? 'positive' : 'negative'}`}>
          {data?.addressGrowthChart.growthRate?.toFixed(1)}%
        </span>
        <span className="period">({data?.addressGrowthChart.timeRange})</span>
      </div>
      <AddressGrowthChart timeRange={timeRange} />
    </div>
  );
}
```