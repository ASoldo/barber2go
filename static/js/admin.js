document.addEventListener("DOMContentLoaded", () => {
  const path = window.location.pathname;
  document.querySelectorAll(".admin-nav a").forEach((link) => {
    const href = link.getAttribute("href") || "";
    if (href !== "/" && path.startsWith(href)) {
      link.classList.add("active");
    }
  });
});
