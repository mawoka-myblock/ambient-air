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

	let measurements: Measurement[] = [];

	interface Measurement {
		temp_p: number; // in hundredths of a degree C
		pressure: number; // in tenths of a Pa
		temp_t: number; // in hundredths of a degree C
		humidity: number; // %
		co2: number; // ppm
		voc: number; // Sensirion index
	}

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

	async function readSamples() {
		measurements = [];
		let readingSamples = true;

		const dataChar = await getChar(SERVICES.MEASUREMENT, CHARS.measureData);
		const countChar = await getChar(SERVICES.MEASUREMENT, CHARS.measureSampleCount);

		/* ---------- register the notification callback ---------- */
		await dataChar.startNotifications();
		const onData = (e: Event) => {
			const value = (e.target as BluetoothRemoteGATTCharacteristic).value;
			if (!value) return;
			const buf = new Uint8Array(value.buffer);

			/*  Each packet is 20 bytes (the Rust struct is marked as 22, but only 20 bytes are defined).
          The device may send several packets in a single burst – we simply loop over all of them. */
			for (let i = 0; i + 20 <= buf.length; i += 20) {
				const view = new DataView(buf.buffer, buf.byteOffset + i, 20);
				const m: Measurement = {
					temp_p: view.getInt32(0, true),
					pressure: view.getUint32(4, true),
					temp_t: view.getInt32(8, true),
					humidity: view.getUint16(12, true),
					co2: view.getInt16(14, true),
					voc: view.getInt32(16, true)
				};
				measurements = [...measurements, m];
			}
		};
		dataChar.addEventListener('characteristicvaluechanged', onData);

		/* ---------- trigger the data burst ---------- */
		const countVal = await countChar.readValue();
		const expected = countVal.getInt16(0, true);
		console.log(`Reading ${expected} samples …`);
		/* (The act of reading the *sampleCount* characteristic triggers the device to send the packets.) */

		/* ---------- stop notifications after a while ---------- */
		setTimeout(() => {
			dataChar.removeEventListener('characteristicvaluechanged', onData);
			readingSamples = false;

			console.log(`${measurements}`);
		}, 2000); // 2 s should be enough for all bursts – tweak if you need more
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
			<button onclick={readSamples}>Read samples</button>

			<!-- <button
				class="rounded bg-emerald-600 px-4 py-2 hover:bg-emerald-500"
				on:click={startSampling}
			>
				Start Measurement Program
			</button> -->
			{#if measurements.length !== 0}
				<p>Read {measurements.length} samples</p>
				{#each measurements as sample, i}
					<p>Temp: {sample.temp_t}</p>
				{/each}
			{/if}
		{:else}
			<button class="rounded bg-blue-600 px-4 py-2 hover:bg-blue-500" onclick={connectDevice}>
				Connect Device
			</button>
		{/if}
	</div>
</div>
