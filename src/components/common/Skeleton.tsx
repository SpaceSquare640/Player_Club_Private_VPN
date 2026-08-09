/** Generic skeleton-screen placeholder shown while a route chunk loads. Shape
 * loosely mirrors the Dashboard's layout (status card, 3-up quick actions,
 * activity list) since that's the most common first paint. */
export function Skeleton() {
  return (
    <div className="space-y-6" data-testid="skeleton" aria-busy="true">
      <div className="h-8 w-48 animate-pulse rounded-lg bg-white/5" />
      <div className="h-28 animate-pulse rounded-2xl bg-white/5" />
      <div className="grid grid-cols-3 gap-3">
        <div className="h-16 animate-pulse rounded-2xl bg-white/5" />
        <div className="h-16 animate-pulse rounded-2xl bg-white/5" />
        <div className="h-16 animate-pulse rounded-2xl bg-white/5" />
      </div>
      <div className="h-32 animate-pulse rounded-2xl bg-white/5" />
    </div>
  );
}
