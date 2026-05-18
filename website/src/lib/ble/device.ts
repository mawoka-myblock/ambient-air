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

type CharDef<T> = {
	service: string;
	char: string;
	decode?: (v: DataView) => T;
	encode?: (v: T) => BufferSource;
};

type NotifyCallback<T> = (value: T) => void;

export interface Measurement {
	temp_p: number; // in hundredths of a degree C
	pressure: number; // in tenths of a Pa
	temp_t: number; // in hundredths of a degree C
	humidity: number; // %
	co2: number; // ppm
	voc: number; // Sensirion index
	ms_offset: number; // Offset since start
}

export const CHAR_DEFS = {
	temperature: {
		service: SERVICES.TEMP,
		char: '561be71a-359d-4964-b64f-7b1c949b092e',
		decode: (v) => v.getFloat32(0, true)
	},
	humidity: {
		service: SERVICES.TEMP,
		char: '13881d03-54b9-4b8c-be9f-8a0eeec6893b',
		decode: (v) => v.getFloat32(0, true)
	},
	pressure: {
		service: SERVICES.PRESSURE,
		char: '7c4b9d53-cbce-409e-bb3d-06d7f9f263d8',
		decode: (v) => v.getFloat32(0, true)
	},
	pressure_temp: {
		service: SERVICES.PRESSURE,
		char: 'a3f6145d-d2eb-46a6-aa41-9644a44bb18e',
		decode: (v) => v.getFloat32(0, true)
	},
	co2: {
		service: SERVICES.CO2,
		char: 'cfb04cf1-8d5b-4223-9ae5-c9e32b2940ab',
		decode: (v) => v.getInt16(0, true)
	},
	voc: {
		service: SERVICES.VOC,
		char: '55697045-6e90-4940-b055-a03f9ae10122',
		decode: (v) => v.getInt16(0, true)
	},
	voc_enabled: {
		service: SERVICES.VOC,
		char: 'a1666baa-2fd2-456b-ab68-8e83395f9f79',
		decode: (v) => v.getUint8(0) === 1,
		encode: (v) => new Uint8Array([(v as boolean) ? 1 : 0])
	},
	battery_level: {
		service: SERVICES.BATTERY,
		char: '00002a19-0000-1000-8000-00805f9b34fb',
		decode: (v) => v.getUint8(0)
	},
	battery_power: {
		service: SERVICES.BATTERY,
		char: '408813df-5dd4-1f87-ec11-cdb001100000',
		decode: (v) => v.getInt16(0, true)
	},
	measure_data: {
		service: SERVICES.MEASUREMENT,
		char: '127ec103-86ea-4e75-9e35-2e0c772d6f85',
		decode: (v) => {
			const buf = new Uint8Array(v.buffer);
			const measurements: Measurement[] = [];
			for (let i = 0; i + 24 <= buf.length; i += 24) {
				const view = new DataView(buf.buffer, buf.byteOffset + i, 24);
				measurements.push({
					temp_p: view.getInt32(0, true),
					pressure: view.getUint32(4, true),
					temp_t: view.getInt32(8, true),
					humidity: view.getUint16(12, true),
					co2: view.getInt16(14, true),
					voc: view.getInt32(16, true),
					ms_offset: view.getUint32(20, true)
				});
			}
			return measurements;
		}
	},
	measure_count: {
		service: SERVICES.MEASUREMENT,
		char: 'd988b5cc-5154-45e2-9815-4d55261950ad',
		decode: (v) => v.getInt16(0, true)
	},
	measure_command: {
		service: SERVICES.MEASUREMENT,
		char: '84b0a39c-c55f-41f3-8797-de87992adc55',
		encode: (v) => new TextEncoder().encode((v as string) + '\0')
	},
	ms_since_boot: {
		service: SERVICES.TIME,
		char: '9525ce8e-3d50-4975-a8e5-64ddea6dfe10',
		decode: (v): number => Number(v.getBigUint64(0, true) / 1000n)
	}
} satisfies Record<string, CharDef<unknown>>;

type CharKey = keyof typeof CHAR_DEFS;
type CharValue<K extends CharKey> = (typeof CHAR_DEFS)[K] extends {
	encode: (value: infer T) => BufferSource;
}
	? T
	: (typeof CHAR_DEFS)[K] extends { decode: (v: DataView) => infer T }
		? T
		: never;

export class AmbientAir {
	device?: BluetoothDevice;
	server?: BluetoothRemoteGATTServer;

	private charCache = new Map<CharKey, BluetoothRemoteGATTCharacteristic>();
	constructor() {}

	public async connect() {
		this.device = await navigator.bluetooth.requestDevice({
			filters: [{ name: 'AmbientAir' }],
			optionalServices: Object.values(SERVICES)
		});
		this.server = await this.device.gatt!.connect();
	}

	private ensureConnected(): BluetoothRemoteGATTServer {
		if (!this.server) throw new Error('Not connected — call connect() first.');
		return this.server;
	}

	async getChar<K extends CharKey>(key: K): Promise<BluetoothRemoteGATTCharacteristic> {
		if (this.charCache.has(key)) return this.charCache.get(key)!;

		const server = this.ensureConnected();
		const { service, char } = CHAR_DEFS[key];
		const svc = await server.getPrimaryService(service);
		const c = await svc.getCharacteristic(char);
		this.charCache.set(key, c);
		return c;
	}

	async read<K extends CharKey>(key: K): Promise<CharValue<K>> {
		const def = CHAR_DEFS[key] as CharDef<unknown>;
		if (!def.decode) throw new Error(`'${key}' has no decode function.`);
		const c = await this.getChar(key);
		const v = await c.readValue();
		return def.decode(v) as CharValue<K>;
	}

	async send<K extends CharKey>(key: K, value: CharValue<K>): Promise<void> {
		const def = CHAR_DEFS[key] as CharDef<unknown>;
		if (!def.encode) throw new Error(`'${key}' has no encode function.`);
		const c = await this.getChar(key);
		await c.writeValue(def.encode(value));
	}

	async subscribe<K extends CharKey>(
		key: K,
		cb: NotifyCallback<CharValue<K>>
	): Promise<() => void> {
		const def = CHAR_DEFS[key] as CharDef<unknown>;
		if (!def.decode) throw new Error(`'${key}' has no decode function.`);
		const c = await this.getChar(key);
		await c.startNotifications();

		const handler = (e: Event) => {
			const v = (e.target as BluetoothRemoteGATTCharacteristic).value!;
			cb(def.decode!(v) as CharValue<K>);
		};

		c.addEventListener('characteristicvaluechanged', handler);

		// Returns an unsubscribe function
		return () => c.removeEventListener('characteristicvaluechanged', handler);
	}
}
