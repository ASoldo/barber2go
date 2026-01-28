document.addEventListener("DOMContentLoaded", () => {
  const container = document.querySelector("[data-status-page]");
  if (!container || !("EventSource" in window)) return;

  const appointmentId = container.dataset.appointmentId;
  if (!appointmentId) return;

  const source = new EventSource(`/status/${encodeURIComponent(appointmentId)}/events`);
  source.addEventListener("update", (event) => {
    let payload;
    try {
      payload = JSON.parse(event.data);
    } catch {
      return;
    }
    if (!payload || payload.appointment_id !== appointmentId) return;

    container.querySelectorAll("[data-field]").forEach((el) => {
      const key = el.dataset.field;
      if (!key) return;
      if (key === "status") {
        if (payload.status) updateStatus(el, payload.status);
        return;
      }
      const value = payload[key];
      if (key === "barber_name" && (!value || value === "")) {
        el.textContent = "Unassigned";
        return;
      }
      if (value === null || value === undefined || value === "") return;
      el.textContent = value;
    });

    if (payload.status) {
      container
        .querySelectorAll(".status")
        .forEach((el) => updateStatus(el, payload.status));
    }
  });
});

const STATUS_CLASSES = ["pending", "accepted", "declined", "completed"];

function updateStatus(el, status) {
  if (!status) return;
  el.textContent = status;
  STATUS_CLASSES.forEach((cls) => el.classList.remove(cls));
  el.classList.add(status);
}
