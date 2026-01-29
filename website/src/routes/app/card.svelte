<script lang="ts">
	let { label, value }: { label: string; value: string } = $props();
</script>

<div class="rounded-xl bg-slate-800 p-4 shadow">
	<div class="text-sm text-slate-400">{label}</div>
	<div class="text-xl font-semibold">{value}</div>
</div>
