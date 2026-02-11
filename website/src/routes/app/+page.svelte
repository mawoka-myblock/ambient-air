<script lang="ts">
	import { connect, getChar, readF32 } from '$lib/ble/device';
	import { SERVICES, CHARS } from '$lib/ble/uuids';
	import Card from './card.svelte';

	let connected = false;

	let temp = 0;
	let humidity = 0;
	let pressure = 0;
	let pres_temp = 0;
	let co2 = 0;
	let voc = 0;
	let battery = 0;
	let voc_enabled = false;
	let power_mw = 0;

	async function startNotify(service: string, char: string, cb) {
		const c = await getChar(service, char);
		await c.startNotifications();
		c.addEventListener('characteristicvaluechanged', (e) => {
			cb(e.target.value);
		});
	}

	async function toggle_voc() {
		const c = await getChar(SERVICES.VOC, CHARS.vocEnabled);
		const value = new Uint8Array([voc_enabled ? 0 : 1]);
		await c.writeValue(value);
		voc_enabled = !voc_enabled;
	}

	async function connectDevice() {
		await connect();

		await startNotify(SERVICES.TEMP, CHARS.temperature, (v) => (temp = readF32(v)));
		await startNotify(SERVICES.TEMP, CHARS.humidity, (v) => (humidity = readF32(v)));
		await startNotify(SERVICES.PRESSURE, CHARS.pressure, (v) => (pressure = readF32(v)));
		await startNotify(
			SERVICES.PRESSURE,
			CHARS.pressure_temperature,
			(v) => (pres_temp = readF32(v))
		);
		await startNotify(SERVICES.CO2, CHARS.co2, (v) => (co2 = v.getInt16(0, true)));
		await startNotify(SERVICES.VOC, CHARS.vocIndex, (v) => (voc = v.getInt16(0, true)));
		await startNotify(SERVICES.BATTERY, CHARS.batteryLevel, (v) => (battery = v.getUint8(0)));
		await startNotify(
			SERVICES.BATTERY,
			CHARS.batteryPower,
			(v) => (power_mw = v.getInt16(0, true))
		);
		const vocEnabledChar = await getChar(SERVICES.VOC, CHARS.vocEnabled);

		const value = await vocEnabledChar.readValue();
		voc_enabled = value.getUint8(0) === 1;

		connected = true;
	}

	async function startSampling() {
		const c = await getChar(SERVICES.MEASUREMENT, CHARS.measureCommand);
		const payload =
			JSON.stringify({
				every_x_seconds: 5,
				samples: 10
			}) + '\0';

		await c.writeValue(new TextEncoder().encode(payload));
	}
</script>

<div class="min-h-screen bg-slate-900 p-6 text-white">
	<div class="mx-auto max-w-xl space-y-6">
		<h1 class="text-3xl font-bold">🌫 Ambient-Air Monitor</h1>

		{#if connected}
			<div class="grid grid-cols-2 gap-4">
				<Card label="Temperature" value={`${temp.toFixed(2)} °C`} />
				<Card label="Humidity" value={`${humidity.toFixed(1)} %`} />
				<Card label="Pressure" value={`${pressure.toFixed(2)} kPa`} />
				<Card label="Pressure Temperature" value={`${pres_temp.toFixed(2)} °C`} />
				<Card label="CO₂" value={`${co2} ppm`} />
				<Card label="VOC Index" value={voc.toString()} />
				<Card label="Battery" value={`${battery}%`} />
				<Card label="Power" value={`${power_mw}mw`} />
			</div>
			<button onclick={toggle_voc}
				>{#if voc_enabled}Disable{:else}Enable{/if} VOC</button
			>

			<!-- <button
				class="rounded bg-emerald-600 px-4 py-2 hover:bg-emerald-500"
				on:click={startSampling}
			>
				Start Measurement Program
			</button> -->
		{:else}
			<button class="rounded bg-blue-600 px-4 py-2 hover:bg-blue-500" onclick={connectDevice}>
				Connect Device
			</button>
		{/if}
	</div>
</div>
