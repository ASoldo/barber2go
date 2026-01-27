class EditableBlock extends HTMLElement {
  connectedCallback() {
    const key = this.dataset.key;
    const title = this.dataset.title || "Content block";
    const original = this.innerHTML.trim();

    this.innerHTML = `
      <div class="cms-block">
        <div class="cms-block-header">
          <div>
            <p class="label">${key}</p>
            <h3>${title}</h3>
          </div>
          <button class="btn light" type="button">Save</button>
        </div>
        <div class="cms-block-body" contenteditable="true"></div>
        <p class="cms-status muted"></p>
      </div>
    `;

    const body = this.querySelector(".cms-block-body");
    const status = this.querySelector(".cms-status");
    const button = this.querySelector("button");

    body.innerHTML = original;

    let dirty = false;
    body.addEventListener("input", () => {
      dirty = true;
      status.textContent = "Unsaved changes";
    });

    button.addEventListener("click", async () => {
      status.textContent = "Saving...";
      const payload = {
        key,
        html: body.innerHTML,
      };

      try {
        const response = await fetch("/admin/cms/save", {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify(payload),
        });

        if (!response.ok) {
          throw new Error("Save failed");
        }

        dirty = false;
        status.textContent = "Saved";
      } catch (err) {
        status.textContent = "Save failed. Try again.";
      }
    });

    window.addEventListener("beforeunload", (event) => {
      if (dirty) {
        event.preventDefault();
        event.returnValue = "";
      }
    });
  }
}

customElements.define("editable-block", EditableBlock);
