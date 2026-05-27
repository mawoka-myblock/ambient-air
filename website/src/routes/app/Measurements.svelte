<script lang="ts">
	import type { Measurement } from '$lib/ble/device';

	import {
		Chart,
		CategoryScale,
		LinearScale,
		PointElement,
		LineElement,
		LineController,
		Legend,
		Tooltip,
		Filler
	} from 'chart.js';
	import { onMount } from 'svelte';

	let chart: Chart | null = null;

	let { measurements }: { measurements: Measurement[] } = $props();

	let chart_canvas: HTMLCanvasElement | undefined = $state();

	const clean_data = (data: Measurement[]): Measurement[] => {
		// clone so we do not mutate original
		const cleaned = [...data];

		// ------------------------------------------------------------
		// 1. Remove trailing empty measurements
		// ------------------------------------------------------------
		// A measurement is considered "empty" if every value is 0
		while (cleaned.length > 0) {
			const last = cleaned.at(-1);

			if (
				last &&
				last.temp_p === 0 &&
				last.pressure === 0 &&
				last.temp_t === 0 &&
				last.humidity === 0 &&
				last.co2 === 0 &&
				last.voc === 0
			) {
				cleaned.pop();
			} else {
				break;
			}
		}

		// ------------------------------------------------------------
		// 2. Replace intermittent CO2/VOC zeros
		// ------------------------------------------------------------
		// CO2 sensor updates slowly, so keep previous valid value
		let last_co2 = 0;
		let last_voc = 0;

		return cleaned.map((dp) => {
			const next = { ...dp };

			if (next.co2 === 0 && last_co2 !== 0) {
				next.co2 = last_co2;
			} else if (next.co2 !== 0) {
				last_co2 = next.co2;
			}

			if (next.voc === 0 && last_voc !== 0) {
				next.voc = last_voc;
			} else if (next.voc !== 0) {
				last_voc = next.voc;
			}

			return next;
		});
	};

	let data = $derived(clean_data(measurements));

	const download_csv = () => {
		console.log(data);
		const headers = [
			'temperature_icp',
			'pressure',
			'temp_aht',
			'humidity',
			'co2',
			'voc',
			'ms_offset'
		];

		const rows = data.map((dp) => [
			dp.temp_p / 100,
			dp.pressure / 1000,
			dp.temp_t / 100,
			dp.humidity,
			dp.co2,
			dp.voc,
			dp.ms_offset
		]);

		const csv = [headers.join(','), ...rows.map((row) => row.join(','))].join('\n');

		const blob = new Blob([csv], { type: 'text/csv;charset=utf-8;' });
		const url = URL.createObjectURL(blob);

		const link = document.createElement('a');
		link.href = url;
		link.download = 'data.csv';

		document.body.appendChild(link);
		link.click();
		document.body.removeChild(link);

		URL.revokeObjectURL(url);
	};

	const make_dataset = (label: string, values: number[], color: string, yAxisID: string) => {
		// hide datasets where every value is 0
		const has_non_zero = values.some((v) => v !== 0);

		return {
			label,
			data: values,
			borderColor: color,
			backgroundColor: color,
			borderWidth: 2,
			pointRadius: 1.5,
			tension: 0.2,
			yAxisID,
			hidden: !has_non_zero
		};
	};

	const render_chart = () => {
		if (!chart_canvas) return;

		chart?.destroy();

		const labels = data.map((dp) => `${(dp.ms_offset / 1000).toFixed(0)}s`);

		chart = new Chart(chart_canvas, {
			type: 'line',
			data: {
				labels,
				datasets: [
					make_dataset(
						'Temperature ICP (°C)',
						data.map((dp) => dp.temp_p / 100),
						'#ef4444',
						'y'
					),

					make_dataset(
						'Temperature AHT (°C)',
						data.map((dp) => dp.temp_t / 100),
						'#f97316',
						'y'
					),

					make_dataset(
						'Humidity (%)',
						data.map((dp) => dp.humidity),
						'#3b82f6',
						'y1'
					),

					make_dataset(
						'Pressure (kPa)',
						data.map((dp) => dp.pressure / 1000),
						'#8b5cf6',
						'y2'
					),

					make_dataset(
						'CO₂ (ppm)',
						data.map((dp) => (dp.co2 ? dp.co2 : 0)),
						'#10b981',
						'y3'
					),

					make_dataset(
						'VOC',
						data.map((dp) => dp.voc),
						'#eab308',
						'y3'
					)
				].filter((ds) => ds.data.some((v) => v !== 0))
			},
			options: {
				responsive: true,
				maintainAspectRatio: false,
				interaction: {
					mode: 'index',
					intersect: false
				},
				// stacked: false,
				plugins: {
					legend: {
						position: 'top',
						onClick: (e, legendItem, legend) => {
							const index = legendItem.datasetIndex!;
							const chart = legend.chart;

							chart.setDatasetVisibility(index, !chart.isDatasetVisible(index));
							chart.update();

							update_scale_visibility(chart);
						}
					}
				},
				transitions: {
					show: {
						animations: {
							tension: { duration: 0 },
							y: { duration: 0 }
						}
					},
					hide: {
						animations: {
							tension: { duration: 0 },
							y: { duration: 0 }
						}
					}
				},
				scales: {
					x: {
						title: {
							display: true,
							text: 'Time'
						}
					},
					y: {
						type: 'linear',
						position: 'left',
						title: {
							display: true,
							text: 'Temperature (°C)'
						}
					},
					y1: {
						type: 'linear',
						position: 'right',
						title: {
							display: true,
							text: 'Humidity (%)'
						},
						grid: {
							drawOnChartArea: false
						}
					},
					y2: {
						type: 'linear',
						position: 'right',
						title: {
							display: true,
							text: 'Pressure (kPa)'
						},
						grid: {
							drawOnChartArea: false
						}
					},
					y3: {
						type: 'linear',
						position: 'right',
						title: {
							display: true,
							text: 'CO2 (ppm)'
						},
						grid: {
							drawOnChartArea: false
						}
					}
				}
			}
		});
	};

	const update_scale_visibility = (chart: Chart) => {
		const datasets = chart.data.datasets;

		const axis_usage: Record<string, boolean> = {};

		for (const ds of datasets) {
			const meta = chart.getDatasetMeta(datasets.indexOf(ds));
			const is_visible = !meta.hidden;

			if (is_visible && ds.yAxisID) {
				axis_usage[ds.yAxisID] = true;
			}
		}

		for (const scale_id of Object.keys(chart.options.scales ?? {})) {
			const scale = chart.options.scales?.[scale_id];

			if (!scale) continue;

			// hide scale if no visible dataset uses it
			scale.display = !!axis_usage[scale_id];
		}

		chart.update();
	};

	onMount(() => {
		Chart.register(
			CategoryScale,
			LinearScale,
			PointElement,
			LineElement,
			LineController,
			Legend,
			Tooltip,
			Filler
		);
		render_chart();
	});
</script>

<div>
	<div class="relative mt-4 h-[60vh] min-h-96 w-full rounded bg-white">
		<canvas bind:this={chart_canvas}></canvas>
	</div>

	<button
		class="mt-4 rounded bg-emerald-600 px-4 py-2 text-sm transition-colors hover:cursor-pointer hover:bg-emerald-500"
		onclick={download_csv}>Download as CSV</button
	>
</div>
