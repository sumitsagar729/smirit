/** [lon, lat] for anything with coordinates (floats arrive from Rust as `number | null`). */
export const lonLat = (o: { lat: number | null; lon: number | null }): [number, number] => [o.lon ?? 0, o.lat ?? 0]
