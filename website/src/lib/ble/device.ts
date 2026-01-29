import { SERVICES } from './uuids';

export let device: BluetoothDevice;
export let server: BluetoothRemoteGATTServer;

export async function connect() {
	device = await navigator.bluetooth.requestDevice({
		filters: [{ name: 'AmbientAir' }],
		optionalServices: Object.values(SERVICES)
	});

	server = await device.gatt!.connect();
}

export async function getChar(service: string, char: string) {
	const s = await server.getPrimaryService(service);
	return await s.getCharacteristic(char);
}

export function readF32(view: DataView) {
	return view.getFloat32(0, true);
}
