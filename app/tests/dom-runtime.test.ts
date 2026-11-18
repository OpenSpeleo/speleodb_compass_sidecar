import { expect, it, vi } from "vitest";

it("supports jsdom window listeners and bubbling DOM events on Bun", () => {
  const listener = vi.fn();
  const button = document.createElement("button");
  document.body.append(button);
  window.addEventListener("click", listener);

  try {
    button.click();
    expect(listener).toHaveBeenCalledOnce();
    expect(listener.mock.calls[0]?.[0].target).toBe(button);

    window.removeEventListener("click", listener);
    button.click();
    expect(listener).toHaveBeenCalledOnce();
  } finally {
    window.removeEventListener("click", listener);
    button.remove();
  }
});
