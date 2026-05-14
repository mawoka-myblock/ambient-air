<script lang="ts">
	import { AmbientAir } from '$lib/ble/device';
	import App from './App.svelte';

	let connected = false;

	let aa: AmbientAir;

	async function connectDevice() {
		aa = new AmbientAir();
		await aa.connect();

		connected = true;
	}
</script>

<div class="min-h-screen bg-slate-900 p-6 text-white">
	<div class="mx-auto max-w-xl space-y-6">
		<h1 class="text-3xl font-bold">🌫 Ambient-Air Monitor</h1>

		{#if connected}
			<App {aa} />
		{:else}
			<button class="rounded bg-blue-600 px-4 py-2 hover:bg-blue-500" onclick={connectDevice}>
				Connect Device
			</button>
		{/if}
	</div>
</div>
