<script lang="ts">
	import type { AmbientAir, Measurement } from '$lib/ble/device';
	import { onMount } from 'svelte';
	import Card from './card.svelte';

	let { aa }: { aa: AmbientAir } = $props();

	let temp = $state(0);
	let humidity = $state(0);
	let pressure = $state(0);
	let pres_temp = $state(0);
	let co2 = $state(0);
	let voc = $state(0);
	let battery = $state(0);
	let voc_enabled = $state(false);
	let power_mw = $state(0);
	let ms_since_boot = $state(0);

	let sampling_data = $state({
		count: 10,
		interval: 1
	});

	let measurements: Measurement[] = $state([]);

	async function toggle_voc() {
		voc_enabled = !voc_enabled;
	}

	async function startSampling(interval: number, count: number) {
		const payload = JSON.stringify({
			every_x_seconds: interval,
			samples: count
		});
		await aa.send('measure_command', payload);
	}

	async function readSamples() {
		measurements = [];

		const unsubscribe = await aa.subscribe('measure_data', (batch) => {
			measurements = [...measurements, ...batch];
		});

		const expected = await aa.read('measure_count');
		console.log(`Reading ${expected} samples…`);

		setTimeout(() => {
			unsubscribe();
			console.log(`Got ${measurements.length} samples`);
		}, 2000);
	}

	onMount(async () => {
		aa.subscribe('battery_level', (v) => (battery = v));
		aa.subscribe('temperature', (v) => (temp = v));
		aa.subscribe('humidity', (v) => (humidity = v));
		aa.subscribe('pressure', (v) => (pressure = v));
		aa.subscribe('pressure_temp', (v) => (pres_temp = v));
		aa.subscribe('co2', (v) => (co2 = v));
		aa.subscribe('voc', (v) => (voc = v));
		aa.subscribe('battery_power', (v) => (power_mw = v));

		// await aa.send('current_time', new Date());
		ms_since_boot = await aa.read('ms_since_boot');
	});
</script>

<div>
	<div class="grid grid-cols-2 gap-4">
		<Card label="Temperature" value={`${temp.toFixed(2)} °C`} />
		<Card label="Humidity" value={`${humidity.toFixed(1)} %`} />
		<Card label="Pressure" value={`${pressure.toFixed(2)} kPa`} />
		<Card label="Pressure Temperature" value={`${pres_temp.toFixed(2)} °C`} />
		<Card label="CO₂" value={`${co2} ppm`} />
		<Card label="VOC Index" value={voc.toString()} />
		<Card label="Battery" value={`${battery}%`} />
		<Card label="Power" value={`${power_mw}mw`} />
		<Card label="Time" value={(ms_since_boot / 1000).toString()} />
	</div>
	<button onclick={toggle_voc}
		>{#if voc_enabled}Disable{:else}Enable{/if} VOC</button
	>
	<button onclick={readSamples}>Read samples</button>

	<div class="space-y-4 rounded-lg border border-slate-700 bg-slate-800 p-4">
		<h2 class="font-semibold text-slate-200">Sampling</h2>
		<div class="grid grid-cols-2 gap-4">
			<label class="space-y-1">
				<span class="text-sm text-slate-400">Interval (seconds)</span>
				<input
					type="number"
					min="1"
					bind:value={sampling_data.interval}
					class="w-full rounded border border-slate-600 bg-slate-900 px-3 py-1.5 text-white focus:border-blue-500 focus:outline-none"
				/>
			</label>
			<label class="space-y-1">
				<span class="text-sm text-slate-400">Sample count</span>
				<input
					type="number"
					min="1"
					bind:value={sampling_data.count}
					class="w-full rounded border border-slate-600 bg-slate-900 px-3 py-1.5 text-white focus:border-blue-500 focus:outline-none"
				/>
			</label>
		</div>
		<button
			onclick={() => startSampling(sampling_data.interval, sampling_data.count)}
			class="rounded bg-emerald-600 px-4 py-2 text-sm transition-colors hover:bg-emerald-500"
		>
			Start Sampling
		</button>
	</div>

	<!-- <button
				class="rounded bg-emerald-600 px-4 py-2 hover:bg-emerald-500"
				on:click={startSampling}
			>
				Start Measurement Program
			</button> -->
	{#if measurements.length !== 0}
		<div class="overflow-x-auto rounded-lg border border-slate-700">
			<table class="w-full text-sm">
				<thead class="bg-slate-800 text-slate-400">
					<tr>
						<th class="px-4 py-2 text-left">Time (ms)</th>
						<th class="px-4 py-2 text-right">Temp (°C)</th>
						<th class="px-4 py-2 text-right">Pressure Temp (°C)</th>
						<th class="px-4 py-2 text-right">Pressure</th>
						<th class="px-4 py-2 text-right">Humidity (%)</th>
						<th class="px-4 py-2 text-right">CO₂ (ppm)</th>
						<th class="px-4 py-2 text-right">VOC</th>
					</tr>
				</thead>
				<tbody class="divide-y divide-slate-700">
					{#each measurements as sample (sample.ms_offset)}
						<tr class="bg-slate-900 transition-colors hover:bg-slate-800">
							<td class="px-4 py-2 text-slate-400">{sample.ms_offset}</td>
							<td class="px-4 py-2 text-right">{(sample.temp_t / 100).toFixed(2)}</td>
							<td class="px-4 py-2 text-right">{(sample.temp_p / 100).toFixed(2)}</td>
							<td class="px-4 py-2 text-right">{(sample.pressure / 10).toFixed(1)} Pa</td>
							<td class="px-4 py-2 text-right">{sample.humidity}%</td>
							<td class="px-4 py-2 text-right">{sample.co2}</td>
							<td class="px-4 py-2 text-right">{sample.voc}</td>
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="text-sm text-slate-400">Read {measurements.length} samples</p>
	{/if}
</div>
