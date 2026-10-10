/* Chuyển đổi giữa các tab, ghi nhớ tab đang mở. */

const TAB_KEY = "bmt.activeTab";

export function initTabs(onSwitch = () => {}) {
  const buttons = [...document.querySelectorAll(".tab-button")];

  function activate(name, persist = true) {
    for (const button of buttons) {
      const active = button.dataset.tab === name;
      button.classList.toggle("is-active", active);
      button.setAttribute("aria-selected", String(active));
    }
    for (const panel of document.querySelectorAll("[role='tabpanel']")) {
      panel.hidden = panel.id !== `panel-${name}`;
    }
    if (persist) localStorage.setItem(TAB_KEY, name);
    onSwitch(name);
  }

  for (const button of buttons) {
    button.addEventListener("click", () => activate(button.dataset.tab));
  }

  activate(localStorage.getItem(TAB_KEY) || "test", false);
  return activate;
}
