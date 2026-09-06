let hideTimer: ReturnType<typeof setTimeout> | null = null;

function ensureContainer(): HTMLDivElement {
  let container = document.querySelector(".action-feedback") as HTMLDivElement | null;
  if (container) return container;

  container = document.createElement("div");
  container.className = "action-feedback";
  document.body.appendChild(container);
  return container;
}

export function show(message: string): void {
  const container = ensureContainer();
  container.textContent = message;
  container.classList.add("is-visible");

  if (hideTimer !== null) {
    clearTimeout(hideTimer);
  }
  hideTimer = setTimeout(() => {
    container.classList.remove("is-visible");
    hideTimer = null;
  }, 1100);
}
