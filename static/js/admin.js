document.addEventListener("DOMContentLoaded", () => {
  const path = window.location.pathname;
  document.querySelectorAll(".admin-nav a").forEach((link) => {
    const href = link.getAttribute("href") || "";
    if (href !== "/" && path.startsWith(href)) {
      link.classList.add("active");
    }
  });

  if ("EventSource" in window) {
    const source = new EventSource("/events");
    source.addEventListener("update", (event) => {
      let payload;
      try {
        payload = JSON.parse(event.data);
      } catch {
        return;
      }
      if (!payload || !payload.appointment_id) {
        return;
      }

      upsertAdminAppointments(payload);
      upsertBarberAppointments(payload);
      updateExistingAppointments(payload);
      scheduleMapSync();
    });
  }

  const adminMenu = document.getElementById("admin-menu");
  const adminOpen = document.querySelector("[data-admin-menu-open]");
  const adminClose = document.querySelector("[data-admin-menu-close]");
  if (adminMenu && adminOpen && adminClose) {
    adminOpen.addEventListener("click", () => adminMenu.showModal());
    adminClose.addEventListener("click", () => adminMenu.close());
    adminMenu.addEventListener("click", (event) => {
      if (event.target === adminMenu) {
        adminMenu.close();
      }
    });
    adminMenu.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => adminMenu.close());
    });
  }

  if (window.L) {
    mapRegistry.admin = initAppointmentMap({
      mapSelector: "#admin-map",
      listSelector: "[data-admin-appointments]",
    });
    mapRegistry.barber = initAppointmentMap({
      mapSelector: "#barber-map",
      listSelector: "[data-barber-appointments]",
    });
  }
});

const STATUS_CLASSES = ["pending", "accepted", "declined", "completed"];
const ZAGREB_CENTER = [45.815, 15.9819];
const mapRegistry = {
  admin: null,
  barber: null,
};
let mapSyncTimer;

function updateExistingAppointments(payload) {
  const targets = document.querySelectorAll(
    `[data-appointment-id="${payload.appointment_id}"]`
  );
  targets.forEach((target) => applyPayload(target, payload));
}

function applyPayload(target, payload) {
  if (payload.status) {
    target.dataset.appointmentStatus = payload.status;
  }
  if (payload.latitude !== null && payload.latitude !== undefined) {
    target.dataset.lat = payload.latitude;
  }
  if (payload.longitude !== null && payload.longitude !== undefined) {
    target.dataset.lon = payload.longitude;
  }
  if (payload.address) {
    target.dataset.appointmentAddress = payload.address;
  }

  target.querySelectorAll("[data-field]").forEach((el) => {
    const key = el.dataset.field;
    if (!key) return;
    if (key === "status") {
      if (payload.status) {
        updateStatus(el, payload.status);
      }
      return;
    }
    let value = payload[key];
    if (value === null || value === undefined || value === "") {
      if (key === "barber_name") {
        value = "Unassigned";
      } else {
        return;
      }
    }
    el.textContent = value;
  });

  if (payload.status) {
    target
      .querySelectorAll(".status")
      .forEach((el) => updateStatus(el, payload.status));
  }
}

function updateStatus(el, status) {
  if (!status) return;
  el.textContent = status;
  STATUS_CLASSES.forEach((cls) => el.classList.remove(cls));
  el.classList.add(status);
}

function upsertAdminAppointments(payload) {
  const list = document.querySelector("[data-admin-appointments]");
  if (!list) return;

  const filter = list.dataset.statusFilter || "";
  const status = payload.status || "";
  const id = payload.appointment_id;
  const row = list.querySelector(`[data-appointment-id="${id}"]`);
  const shouldShow = !filter || (status && status === filter);

  if (row) {
    if (filter && status && status !== filter) {
      row.remove();
      toggleEmptyState(list);
      scheduleMapSync();
      return;
    }
    applyPayload(row, payload);
    scheduleMapSync();
    return;
  }

  if (!shouldShow) return;

  const newRow = buildAdminRow(payload);
  list.prepend(newRow);
  toggleEmptyState(list);
  scheduleMapSync();
}

function buildAdminRow(payload) {
  const row = document.createElement("a");
  row.className = "stack-card";
  row.href = `/admin/appointments/${payload.appointment_id}`;
  row.dataset.appointmentId = payload.appointment_id;
  if (payload.status) {
    row.dataset.appointmentStatus = payload.status;
  }
  row.dataset.appointmentUrl = row.href;
  if (payload.address) {
    row.dataset.appointmentAddress = payload.address;
  }
  if (payload.latitude !== null && payload.latitude !== undefined) {
    row.dataset.lat = payload.latitude;
  }
  if (payload.longitude !== null && payload.longitude !== undefined) {
    row.dataset.lon = payload.longitude;
  }

  row.append(
    buildStackField("client", "Client", payload.client_name || "New client", "client_name"),
    buildStackField("service", "Service", payload.service || "Service", "service"),
    buildStackField("schedule", "Schedule", payload.scheduled_for || "TBD", "scheduled_for"),
    buildStackField("barber", "Barber", payload.barber_name || "Unassigned", "barber_name"),
    buildStatusField(payload.status || "pending")
  );

  return row;
}

function toggleEmptyState(container) {
  const empty = container.querySelector("[data-empty-state]");
  if (!empty) return;
  const hasRows = container.querySelectorAll("[data-appointment-id]").length > 0;
  empty.style.display = hasRows ? "none" : "";
}

function upsertBarberAppointments(payload) {
  const stack = document.querySelector("[data-barber-appointments]");
  if (!stack) return;

  const barberId = stack.dataset.barberId || "";
  const id = payload.appointment_id;
  const status = payload.status || "";
  const assignedId = payload.barber_id || "";
  const isPendingUnassigned = status === "pending" && !assignedId;
  const isAssignedToMe = assignedId && assignedId === barberId;
  const shouldShow = isPendingUnassigned || isAssignedToMe;
  const card = stack.querySelector(`[data-appointment-id="${id}"]`);

  if (card) {
    if (!shouldShow) {
      card.remove();
      toggleEmptyState(stack);
      scheduleMapSync();
      return;
    }
    applyPayload(card, payload);
    scheduleMapSync();
    return;
  }

  if (!shouldShow) return;

  const newCard = buildBarberCard(payload);
  stack.prepend(newCard);
  toggleEmptyState(stack);
  scheduleMapSync();
}

function buildBarberCard(payload) {
  const card = document.createElement("div");
  card.className = "card appointment-card";
  card.id = `appointment-${payload.appointment_id}`;
  card.dataset.appointmentId = payload.appointment_id;
  if (payload.status) {
    card.dataset.appointmentStatus = payload.status;
  }
  card.dataset.appointmentUrl = `#appointment-${payload.appointment_id}`;
  if (payload.latitude !== null && payload.latitude !== undefined) {
    card.dataset.lat = payload.latitude;
  }
  if (payload.longitude !== null && payload.longitude !== undefined) {
    card.dataset.lon = payload.longitude;
  }

  const header = document.createElement("div");
  header.className = "card-header";

  const titleWrap = document.createElement("div");
  const name = document.createElement("h3");
  name.dataset.field = "client_name";
  name.textContent = payload.client_name || "New client";
  const meta = document.createElement("p");
  meta.className = "muted";
  const service = document.createElement("span");
  service.dataset.field = "service";
  service.textContent = payload.service || "Service";
  const dot = document.createTextNode(" · ");
  const schedule = document.createElement("span");
  schedule.dataset.field = "scheduled_for";
  schedule.textContent = payload.scheduled_for || "TBD";
  meta.append(service, dot, schedule);
  titleWrap.append(name, meta);

  const status = document.createElement("span");
  status.dataset.field = "status";
  status.className = "status";
  const statusValue = payload.status || "pending";
  status.textContent = statusValue;
  status.classList.add(statusValue);

  header.append(titleWrap, status);

  const body = document.createElement("div");
  body.className = "card-body";
  body.append(
    buildLabeledField("Address:", "address", payload.address || "TBD"),
    buildLabeledField("Phone:", "client_phone", payload.client_phone || "TBD")
  );

  if (payload.client_email) {
    body.append(
      buildLabeledField("Email:", "client_email", payload.client_email)
    );
  }

  if (payload.notes) {
    body.append(buildLabeledField("Notes:", "notes", payload.notes));
  }

  const actions = document.createElement("div");
  actions.className = "card-actions";
  actions.append(
    buildStatusForm(payload.appointment_id, "accepted", "Accept", "primary"),
    buildStatusForm(payload.appointment_id, "completed", "Complete", "light"),
    buildStatusForm(payload.appointment_id, "declined", "Decline", "ghost")
  );

  card.append(header, body, actions);
  return card;
}

function buildStackField(icon, label, value, field) {
  const wrapper = document.createElement("div");
  wrapper.className = "stack-field";

  const labelEl = document.createElement("span");
  labelEl.className = "stack-label";
  labelEl.append(createIcon(icon));
  labelEl.append(document.createTextNode(label));

  const valueEl = document.createElement("span");
  if (field) {
    valueEl.dataset.field = field;
  }
  valueEl.textContent = value;

  wrapper.append(labelEl, valueEl);
  return wrapper;
}

function buildStatusField(value) {
  const wrapper = document.createElement("div");
  wrapper.className = "stack-field status-field";

  const labelEl = document.createElement("span");
  labelEl.className = "stack-label";
  labelEl.append(createIcon("status"));
  labelEl.append(document.createTextNode("Status"));

  const status = document.createElement("span");
  status.dataset.field = "status";
  status.className = "status";
  status.textContent = value;
  status.classList.add(value);

  wrapper.append(labelEl, status);
  return wrapper;
}

function createIcon(type) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("aria-hidden", "true");
  svg.classList.add("stack-icon");

  const make = (tag) => document.createElementNS("http://www.w3.org/2000/svg", tag);
  if (type === "client") {
    const circle = make("circle");
    circle.setAttribute("cx", "12");
    circle.setAttribute("cy", "8");
    circle.setAttribute("r", "3.2");
    const path = make("path");
    path.setAttribute("d", "M4 20c0-4 4-6 8-6s8 2 8 6");
    svg.append(circle, path);
  } else if (type === "service") {
    const c1 = make("circle");
    c1.setAttribute("cx", "8");
    c1.setAttribute("cy", "8");
    c1.setAttribute("r", "2");
    const c2 = make("circle");
    c2.setAttribute("cx", "8");
    c2.setAttribute("cy", "16");
    c2.setAttribute("r", "2");
    const p1 = make("path");
    p1.setAttribute("d", "M10 9l10 6");
    const p2 = make("path");
    p2.setAttribute("d", "M10 15l10-6");
    svg.append(c1, c2, p1, p2);
  } else if (type === "schedule") {
    const rect = make("rect");
    rect.setAttribute("x", "4");
    rect.setAttribute("y", "6");
    rect.setAttribute("width", "16");
    rect.setAttribute("height", "14");
    rect.setAttribute("rx", "2");
    const p1 = make("path");
    p1.setAttribute("d", "M4 10h16");
    const p2 = make("path");
    p2.setAttribute("d", "M8 4v4");
    const p3 = make("path");
    p3.setAttribute("d", "M16 4v4");
    svg.append(rect, p1, p2, p3);
  } else if (type === "barber") {
    const circle = make("circle");
    circle.setAttribute("cx", "9");
    circle.setAttribute("cy", "8");
    circle.setAttribute("r", "3");
    const p1 = make("path");
    p1.setAttribute("d", "M2 20c0-4 4-6 7-6");
    const p2 = make("path");
    p2.setAttribute("d", "M14 16l2 2 4-4");
    svg.append(circle, p1, p2);
  } else {
    const circle = make("circle");
    circle.setAttribute("cx", "12");
    circle.setAttribute("cy", "12");
    circle.setAttribute("r", "9");
    const path = make("path");
    path.setAttribute("d", "M8 12l3 3 5-5");
    svg.append(circle, path);
  }

  return svg;
}

function buildLabeledField(label, field, value) {
  const p = document.createElement("p");
  const strong = document.createElement("strong");
  strong.textContent = label;
  const span = document.createElement("span");
  span.dataset.field = field;
  span.textContent = value;
  p.append(strong, " ", span);
  return p;
}

function buildStatusForm(appointmentId, status, label, buttonClass) {
  const form = document.createElement("form");
  form.method = "post";
  form.action = `/barber/appointments/${appointmentId}/status`;

  const input = document.createElement("input");
  input.type = "hidden";
  input.name = "status";
  input.value = status;

  const button = document.createElement("button");
  button.type = "submit";
  button.className = `btn ${buttonClass}`;
  button.textContent = label;

  form.append(input, button);
  return form;
}

function scheduleMapSync() {
  clearTimeout(mapSyncTimer);
  mapSyncTimer = setTimeout(() => {
    if (mapRegistry.admin) {
      syncMarkers(mapRegistry.admin);
    }
    if (mapRegistry.barber) {
      syncMarkers(mapRegistry.barber);
    }
  }, 200);
}

function initAppointmentMap({ mapSelector, listSelector }) {
  const mapEl = document.querySelector(mapSelector);
  const listEl = document.querySelector(listSelector);
  if (!mapEl || !listEl || !window.L) return null;

  const map = L.map(mapEl).setView(ZAGREB_CENTER, 12);
  L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
    maxZoom: 19,
    attribution: "© OpenStreetMap",
  }).addTo(map);

  const layer = L.layerGroup().addTo(map);
  const state = {
    map,
    layer,
    listEl,
    markers: new Map(),
    fitted: false,
  };

  syncMarkers(state);
  return state;
}

function syncMarkers(state) {
  if (!state || !state.listEl) return;
  const items = state.listEl.querySelectorAll("[data-appointment-id]");
  const seen = new Set();

  items.forEach((el) => {
    const id = el.dataset.appointmentId;
    const lat = parseFloat(el.dataset.lat);
    const lon = parseFloat(el.dataset.lon);
    if (!id || !Number.isFinite(lat) || !Number.isFinite(lon)) return;

    seen.add(id);
    let marker = state.markers.get(id);
    if (!marker) {
      marker = L.marker([lat, lon]).addTo(state.layer);
      state.markers.set(id, marker);
    } else {
      marker.setLatLng([lat, lon]);
    }

    const popup = buildPopupContent(el);
    if (popup) {
      marker.bindPopup(popup, { closeButton: true, autoClose: true });
    }

    bindMarkerToElement(marker, el);
  });

  for (const [id, marker] of state.markers.entries()) {
    if (!seen.has(id)) {
      state.layer.removeLayer(marker);
      state.markers.delete(id);
    }
  }

  if (!state.fitted && state.markers.size > 0) {
    const bounds = L.latLngBounds(
      Array.from(state.markers.values()).map((marker) => marker.getLatLng())
    );
    state.map.fitBounds(bounds, { padding: [40, 40] });
    state.fitted = true;
  }
}

function buildPopupContent(el) {
  const client = getFieldText(el, "client_name") || "Appointment";
  const service = getFieldText(el, "service");
  const scheduled = getFieldText(el, "scheduled_for");
  const status = getFieldText(el, "status");
  const barber = getFieldText(el, "barber_name");
  const address = getFieldText(el, "address");
  const url = getAppointmentUrl(el);

  const lines = [
    `<strong>${escapeHtml(client)}</strong>`,
    service ? `${escapeHtml(service)}` : null,
    scheduled ? `Scheduled: ${escapeHtml(scheduled)}` : null,
    status ? `Status: ${escapeHtml(status)}` : null,
    barber ? `Barber: ${escapeHtml(barber)}` : null,
    address ? `Address: ${escapeHtml(address)}` : null,
    url
      ? `<a class="map-popup-btn" href="${escapeHtml(url)}">Open appointment</a>`
      : null,
  ].filter(Boolean);

  return `<div class="map-popup">${lines.join("<br />")}</div>`;
}

function getFieldText(el, field) {
  const target = el.querySelector(`[data-field="${field}"]`);
  if (target) return target.textContent.trim();
  if (field === "address") {
    return el.dataset.appointmentAddress || "";
  }
  return "";
}

function getAppointmentUrl(el) {
  return (
    el.dataset.appointmentUrl ||
    el.getAttribute("href") ||
    ""
  );
}

function bindMarkerToElement(marker, el) {
  if (!el.dataset.markerBound) {
    el.dataset.markerBound = "true";
    el.addEventListener("mouseenter", () => marker.openPopup());
    el.addEventListener("mouseleave", () => marker.closePopup());
  }

  marker.off("click");
  marker.on("click", () => marker.openPopup());
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
