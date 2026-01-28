document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll("[data-stagger]").forEach((group) => {
    Array.from(group.children).forEach((child, index) => {
      child.style.setProperty("--i", index);
    });
  });

  const dialog = document.getElementById("mobile-menu");
  const openButton = document.querySelector("[data-menu-open]");
  const closeButton = document.querySelector("[data-menu-close]");
  if (dialog && openButton && closeButton) {
    openButton.addEventListener("click", () => dialog.showModal());
    closeButton.addEventListener("click", () => dialog.close());
    dialog.addEventListener("click", (event) => {
      if (event.target === dialog) {
        dialog.close();
      }
    });
    dialog.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => dialog.close());
    });
  }

  if ("serviceWorker" in navigator) {
    navigator.serviceWorker.register("/sw.js").catch(() => undefined);
  }

  const bookingForm = document.querySelector("[data-booking-form]");
  const vapidKey = document.querySelector("meta[name=\"vapid-public-key\"]")?.content || "";
  if (bookingForm && vapidKey) {
    const hiddenField = bookingForm.querySelector("#push_subscription");
    let submitting = false;

    bookingForm.addEventListener("submit", async (event) => {
      if (submitting || !hiddenField) {
        return;
      }

      if (!("Notification" in window) || !("serviceWorker" in navigator) || !("PushManager" in window)) {
        return;
      }

      if (hiddenField.value) {
        return;
      }

      event.preventDefault();
      submitting = true;

      try {
        const permission = await Notification.requestPermission();
        if (permission === "granted") {
          const registration = await navigator.serviceWorker.ready;
          let subscription = await registration.pushManager.getSubscription();
          if (!subscription) {
            subscription = await registration.pushManager.subscribe({
              userVisibleOnly: true,
              applicationServerKey: urlBase64ToUint8Array(vapidKey),
            });
          }
          hiddenField.value = JSON.stringify(subscription);
        }
      } catch (err) {
        console.warn("Push setup failed", err);
      }

      bookingForm.submit();
    });
  }

  const notificationToggle = document.querySelector("[data-notification-toggle]");
  const helper = document.querySelector("[data-notification-helper]");
  if (notificationToggle && !vapidKey) {
    const label = notificationToggle.querySelector("[data-notification-label]");
    const indicator = notificationToggle.querySelector("[data-notification-indicator]");
    if (label) label.textContent = "Notifications unavailable";
    if (indicator) indicator.classList.add("blocked");
    if (helper) {
      helper.textContent = "Set VAPID keys and use HTTPS or localhost to enable notifications.";
    }
    notificationToggle.addEventListener("click", () => {
      if (helper) {
        helper.textContent = "Set VAPID keys and use HTTPS or localhost to enable notifications.";
      }
    });
  }

  if (notificationToggle && vapidKey) {
    const label = notificationToggle.querySelector("[data-notification-label]");
    const indicator = notificationToggle.querySelector("[data-notification-indicator]");
    const appointmentId = notificationToggle.dataset.appointmentId || "";
    const subscribeUrl = notificationToggle.dataset.subscribeUrl || "";

    const setState = (state, text, helpText) => {
      if (label && text) {
        label.textContent = text;
      }
      if (helper && helpText !== undefined) {
        helper.textContent = helpText;
      }
      if (!indicator) return;
      indicator.classList.remove("on", "blocked");
      if (state === "enabled") {
        indicator.classList.add("on");
      } else if (state === "blocked") {
        indicator.classList.add("blocked");
      }
    };

    const sendSubscription = async (subscription) => {
      if (!subscribeUrl) return;
      await fetch(subscribeUrl, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(subscription),
      });
    };

    const refreshState = async () => {
      if (!("Notification" in window) || !("serviceWorker" in navigator) || !("PushManager" in window)) {
        setState("blocked", "Notifications unavailable", "This browser does not support push notifications.");
        return;
      }

      if (Notification.permission === "denied") {
        setState("blocked", "Notifications blocked", "Allow notifications in your browser settings.");
        return;
      }

      if (Notification.permission === "granted") {
        try {
          const registration = await navigator.serviceWorker.ready;
          const subscription = await registration.pushManager.getSubscription();
          if (subscription) {
            await sendSubscription(subscription);
            setState("enabled", "Notifications enabled", "You'll receive status updates on this device.");
            return;
          }
        } catch {
          setState("blocked", "Notifications unavailable", "We couldn't access the push service.");
          return;
        }
      }

      setState("idle", "Enable notifications", "");
    };

    refreshState();

    notificationToggle.addEventListener("click", async () => {
      if (!appointmentId) return;
      if (!("Notification" in window) || !("serviceWorker" in navigator) || !("PushManager" in window)) {
        setState("blocked", "Notifications unavailable", "This browser does not support push notifications.");
        return;
      }

      if (Notification.permission === "denied") {
        setState("blocked", "Notifications blocked", "Allow notifications in your browser settings.");
        return;
      }

      try {
        const permission = await Notification.requestPermission();
        if (permission !== "granted") {
          setState("blocked", "Notifications blocked", "Allow notifications in your browser settings.");
          return;
        }

        const registration = await navigator.serviceWorker.ready;
        let subscription = await registration.pushManager.getSubscription();
        if (!subscription) {
          subscription = await registration.pushManager.subscribe({
            userVisibleOnly: true,
            applicationServerKey: urlBase64ToUint8Array(vapidKey),
          });
        }
        await sendSubscription(subscription);
        setState("enabled", "Notifications enabled", "You'll receive status updates on this device.");
      } catch (err) {
        console.warn("Notifications failed", err);
        setState("blocked", "Notifications unavailable", "We couldn't access the push service.");
      }
    });
  }
});

function urlBase64ToUint8Array(base64String) {
  const padding = "=".repeat((4 - (base64String.length % 4)) % 4);
  const base64 = (base64String + padding)
    .replace(/-/g, "+")
    .replace(/_/g, "/");
  const rawData = window.atob(base64);
  return Uint8Array.from([...rawData].map((char) => char.charCodeAt(0)));
}
