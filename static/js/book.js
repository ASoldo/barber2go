const addressInput = document.getElementById("address");
const suggestions = document.getElementById("address-suggestions");
const latField = document.getElementById("latitude");
const lonField = document.getElementById("longitude");
const mapContainer = document.getElementById("map");
let allowAutoFill = true;
let reverseTimer;
let activeController;
const resultCache = new Map();

if (mapContainer && window.L) {
  const defaultCenter = [45.815, 15.9819];
  const map = L.map("map").setView(defaultCenter, 13);
  L.tileLayer("https://{s}.tile.openstreetmap.org/{z}/{x}/{y}.png", {
    maxZoom: 19,
    attribution: "© OpenStreetMap",
  }).addTo(map);

  let marker = L.marker(defaultCenter, { draggable: true }).addTo(map);

  const updateLatLon = (lat, lon) => {
    if (!latField || !lonField) return;
    latField.value = lat.toFixed(6);
    lonField.value = lon.toFixed(6);
  };

  const reverseLookup = (lat, lon) => {
    if (!addressInput) return;
    if (!allowAutoFill && addressInput.value.trim()) return;
    clearTimeout(reverseTimer);
    reverseTimer = setTimeout(async () => {
      try {
        const url = new URL("https://nominatim.openstreetmap.org/reverse");
        url.searchParams.set("format", "jsonv2");
        url.searchParams.set("addressdetails", "1");
        url.searchParams.set("lat", lat.toString());
        url.searchParams.set("lon", lon.toString());
        url.searchParams.set("zoom", "18");
        const response = await fetch(url.toString(), {
          headers: { "Accept-Language": "hr" },
        });
        if (!response.ok) return;
        const result = await response.json();
        if (result) {
          const formatted = formatAddress(result);
          if (formatted) {
            addressInput.value = formatted;
          }
          allowAutoFill = true;
        }
      } catch {
        return;
      }
    }, 500);
  };

  updateLatLon(defaultCenter[0], defaultCenter[1]);
  reverseLookup(defaultCenter[0], defaultCenter[1]);

  marker.on("dragend", () => {
    const pos = marker.getLatLng();
    updateLatLon(pos.lat, pos.lng);
    allowAutoFill = true;
    reverseLookup(pos.lat, pos.lng);
  });

  if ("geolocation" in navigator) {
    navigator.geolocation.getCurrentPosition(
      (position) => {
        const lat = position.coords.latitude;
        const lon = position.coords.longitude;
        marker.setLatLng([lat, lon]);
        map.setView([lat, lon], 14);
        updateLatLon(lat, lon);
        allowAutoFill = true;
        reverseLookup(lat, lon);
      },
      () => undefined,
      { enableHighAccuracy: false, timeout: 5000 }
    );
  }

  map.on("click", (event) => {
    marker.setLatLng(event.latlng);
    updateLatLon(event.latlng.lat, event.latlng.lng);
    allowAutoFill = true;
    reverseLookup(event.latlng.lat, event.latlng.lng);
  });

  let debounceTimer;
  if (addressInput && suggestions) {
    addressInput.addEventListener("input", () => {
      allowAutoFill = false;
    });
    addressInput.addEventListener("input", () => {
      clearTimeout(debounceTimer);
      const query = addressInput.value.trim();
      if (query.length < 2) {
        suggestions.classList.remove("open");
        suggestions.innerHTML = "";
        return;
      }
      debounceTimer = setTimeout(() => searchAddress(query), 250);
    });
    addressInput.addEventListener("blur", () => {
      setTimeout(() => {
        suggestions.classList.remove("open");
      }, 200);
    });
  }

  async function searchAddress(query) {
    const normalized = query.toLowerCase();
    if (resultCache.has(normalized)) {
      renderSuggestions(resultCache.get(normalized));
      return;
    }

    if (activeController) {
      activeController.abort();
    }
    activeController = new AbortController();

    try {
      const url = new URL("https://nominatim.openstreetmap.org/search");
      url.searchParams.set("format", "jsonv2");
      url.searchParams.set("q", query);
      url.searchParams.set("limit", "8");
      url.searchParams.set("addressdetails", "1");
      url.searchParams.set("dedupe", "1");
      url.searchParams.set("countrycodes", "hr");
      url.searchParams.set("viewbox", "15.81,45.93,16.12,45.70");
      url.searchParams.set("bounded", "1");

      const response = await fetch(url.toString(), {
        headers: { "Accept-Language": "hr" },
        signal: activeController.signal,
      });
      if (!response.ok) throw new Error("Search failed");
      const results = await response.json();
      resultCache.set(normalized, results);
      renderSuggestions(results);
    } catch (err) {
      if (err?.name === "AbortError") {
        return;
      }
      if (suggestions) {
        suggestions.classList.remove("open");
        suggestions.innerHTML = "";
      }
    }
  }

  function renderSuggestions(results) {
    if (!suggestions) return;
    if (!Array.isArray(results) || results.length === 0) {
      suggestions.classList.remove("open");
      suggestions.innerHTML = "";
      return;
    }
    suggestions.classList.add("open");
    suggestions.innerHTML = "";

    results.forEach((place) => {
      const item = document.createElement("div");
      item.className = "suggestion-item";
      const formatted = formatAddress(place);
      const secondary = formatSecondary(place);
      item.innerHTML = `<strong>${escapeHtml(formatted || place.display_name)}</strong>${secondary ? `<span>${escapeHtml(secondary)}</span>` : ""}`;
      item.addEventListener("click", () => {
        const lat = parseFloat(place.lat);
        const lon = parseFloat(place.lon);
        marker.setLatLng([lat, lon]);
        map.setView([lat, lon], 15);
        updateLatLon(lat, lon);
        addressInput.value = formatted || place.display_name;
        addressInput.focus();
        addressInput.setSelectionRange(addressInput.value.length, addressInput.value.length);
        allowAutoFill = true;
        suggestions.classList.remove("open");
        suggestions.innerHTML = "";
      });
      suggestions.appendChild(item);
    });
  }
}

function formatAddress(place) {
  const address = place?.address || {};
  const road = address.road || address.pedestrian || address.footway || address.path;
  const house = address.house_number || "";
  const locality =
    address.city ||
    address.town ||
    address.village ||
    address.municipality ||
    address.county;
  const amenity = address.amenity || address.building;
  const line =
    road || amenity
      ? `${road || amenity}${house ? ` ${house}` : ""}`
      : place?.name || "";
  const city = locality ? `, ${locality}` : "";
  return line ? `${line}${city}` : place?.display_name || "";
}

function formatSecondary(place) {
  const address = place?.address || {};
  const neighborhood = address.neighbourhood || address.suburb;
  const city =
    address.city ||
    address.town ||
    address.village ||
    address.municipality ||
    address.county;
  if (neighborhood && city) {
    return `${neighborhood}, ${city}`;
  }
  return neighborhood || city || "";
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/\"/g, "&quot;")
    .replace(/'/g, "&#39;");
}
