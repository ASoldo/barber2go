document.addEventListener("DOMContentLoaded", () => {
  document.querySelectorAll("[data-stagger]").forEach((group) => {
    Array.from(group.children).forEach((child, index) => {
      child.style.setProperty("--i", index);
    });
  });
});
