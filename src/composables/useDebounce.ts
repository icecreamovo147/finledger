import { ref, onScopeDispose } from "vue";

/**
 * Debounce a side-effecting function. The returned `run()` is debounced;
 * `flush()` immediately invokes `fn` and cancels any pending invocation.
 *
 * The internal timer is automatically cleared when the component scope
 * is disposed, preventing callbacks after unmount.
 */
export function useDebounce<T extends (...args: any[]) => void>(
  fn: T,
  delayMs: number,
) {
  const timer = ref<ReturnType<typeof setTimeout> | null>(null);

  function clear() {
    if (timer.value != null) {
      clearTimeout(timer.value);
      timer.value = null;
    }
  }

  function run(...args: Parameters<T>) {
    clear();
    timer.value = setTimeout(() => {
      timer.value = null;
      fn(...args);
    }, delayMs);
  }

  function flush(...args: Parameters<T>) {
    clear();
    fn(...args);
  }

  onScopeDispose(() => {
    clear();
  });

  return { run, flush };
}
