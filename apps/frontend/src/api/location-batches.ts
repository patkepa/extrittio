/** Bound request size and concurrency independently of fleet size. */
export async function loadLocationBatches<T>(
  deviceIds: readonly string[],
  load: (ids: string[]) => Promise<T[]>,
): Promise<T[]> {
  const ids = [...new Set(deviceIds)].sort();
  const chunks: string[][] = [];
  for (let start = 0; start < ids.length; start += 500) chunks.push(ids.slice(start, start + 500));
  const results: T[][] = new Array(chunks.length);
  let next = 0;
  let failed = false;
  await Promise.all(
    Array.from({ length: Math.min(3, chunks.length) }, async () => {
      while (!failed && next < chunks.length) {
        const index = next++;
        try {
          results[index] = await load(chunks[index]!);
        } catch (error) {
          failed = true;
          throw error;
        }
      }
    }),
  );
  return results.flat();
}
