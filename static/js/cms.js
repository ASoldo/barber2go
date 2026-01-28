class EditableBlock extends HTMLElement {
  connectedCallback() {
    const key = this.dataset.key;
    const title = this.dataset.title || "Content block";
    const original = this.innerHTML.trim();

    this.innerHTML = `
      <div class="cms-block">
        <div class="cms-block-header">
          <div>
            <h3>${title}</h3>
          </div>
          <button class="btn light" type="button">Save</button>
        </div>
        <div class="cms-toolbar" role="toolbar" aria-label="Content tools">
          <button type="button" data-command="bold" title="Bold"><strong>B</strong></button>
          <button type="button" data-command="italic" title="Italic"><em>I</em></button>
          <button type="button" data-command="underline" title="Underline"><span class="underline">U</span></button>
          <div class="divider"></div>
          <button type="button" data-command="formatBlock" data-value="h1" title="Heading 1">H1</button>
          <button type="button" data-command="formatBlock" data-value="h2" title="Heading 2">H2</button>
          <button type="button" data-command="formatBlock" data-value="p" title="Paragraph">P</button>
          <div class="divider"></div>
          <button type="button" data-command="insertUnorderedList" title="Bulleted list">• List</button>
          <button type="button" data-command="insertOrderedList" title="Numbered list">1. List</button>
          <button type="button" data-command="blockquote" title="Quote">Quote</button>
          <div class="divider"></div>
          <button type="button" data-command="createLink" title="Link">Link</button>
          <button type="button" data-command="removeFormat" title="Clear formatting">Clear</button>
        </div>
        <div class="cms-block-body" contenteditable="true" data-placeholder="Write content..."></div>
        <p class="cms-status muted"></p>
      </div>
    `;

    const body = this.querySelector(".cms-block-body");
    const status = this.querySelector(".cms-status");
    const button = this.querySelector(".cms-block-header .btn");
    const toolbar = this.querySelector(".cms-toolbar");

    body.innerHTML = original;

    let dirty = false;
    let lastContent = body.innerHTML.trim();

    const setButtonState = (state) => {
      if (!button) return;
      button.classList.remove("cms-save-dirty", "cms-save-saved");
      if (state === "hidden") {
        button.classList.add("is-hidden");
        return;
      }
      button.classList.remove("is-hidden");
      if (state === "dirty") {
        button.classList.add("cms-save-dirty");
      }
      if (state === "saved") {
        button.classList.add("cms-save-saved");
        setTimeout(() => {
          button.classList.remove("cms-save-saved");
          setButtonState("hidden");
        }, 1000);
      }
    };

    const markDirty = () => {
      const current = body.innerHTML.trim();
      if (current === lastContent) {
        return;
      }
      dirty = true;
      status.textContent = "Unsaved changes";
      setButtonState("dirty");
    };

    setButtonState("hidden");

    body.addEventListener("input", () => {
      markDirty();
    });

    body.addEventListener("focus", () => {
      document.execCommand("defaultParagraphSeparator", false, "p");
    });

    toolbar.addEventListener("click", (event) => {
      const target = event.target.closest("button[data-command]");
      if (!target) return;
      event.preventDefault();
      body.focus();
      const command = target.dataset.command;
      if (command === "createLink") {
        const url = prompt("Enter a URL", "https://");
        if (url) {
          if (document.queryCommandSupported("createLink")) {
            document.execCommand("createLink", false, url);
          } else {
            surroundSelectionWithLink(url);
          }
          markDirty();
        }
        return;
      }
      if (command === "formatBlock") {
        const value = target.dataset.value || "p";
        document.execCommand("formatBlock", false, value);
        markDirty();
        return;
      }
      if (command === "blockquote") {
        toggleBlockquote(body);
        markDirty();
        return;
      }
      if (command === "removeFormat") {
        if (document.queryCommandSupported("removeFormat")) {
          document.execCommand("removeFormat", false, null);
        }
        cleanupInlineFormatting(body);
        markDirty();
        return;
      }
      if (command === "bold" || command === "italic" || command === "underline") {
        toggleInlineStyle(command);
        markDirty();
        return;
      }
      document.execCommand(command, false, null);
      markDirty();
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
        lastContent = body.innerHTML.trim();
        status.textContent = "Saved";
        setTimeout(() => {
          if (!dirty) {
            status.textContent = "";
          }
        }, 3000);
        setButtonState("saved");
      } catch (err) {
        status.textContent = "Save failed. Try again.";
        setButtonState("dirty");
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

function toggleInlineStyle(command) {
  if (document.queryCommandSupported(command)) {
    document.execCommand(command, false, null);
    return;
  }

  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  if (range.collapsed) return;
  const tag = command === "bold" ? "strong" : command === "italic" ? "em" : "u";
  const wrapper = document.createElement(tag);
  wrapper.appendChild(range.extractContents());
  range.insertNode(wrapper);
  selection.removeAllRanges();
  const newRange = document.createRange();
  newRange.selectNodeContents(wrapper);
  selection.addRange(newRange);
}

function toggleBlockquote(container) {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  let node = selection.anchorNode;
  if (!node) return;
  if (node.nodeType === Node.TEXT_NODE) {
    node = node.parentElement;
  }
  const blockquote = node.closest("blockquote");
  if (blockquote) {
    const parent = blockquote.parentNode;
    while (blockquote.firstChild) {
      parent.insertBefore(blockquote.firstChild, blockquote);
    }
    parent.removeChild(blockquote);
    return;
  }
  if (document.queryCommandSupported("formatBlock")) {
    document.execCommand("formatBlock", false, "blockquote");
  } else {
    const wrapper = document.createElement("blockquote");
    wrapper.appendChild(selection.getRangeAt(0).extractContents());
    selection.getRangeAt(0).insertNode(wrapper);
  }
}

function cleanupInlineFormatting(container) {
  const tags = ["b", "strong", "i", "em", "u", "span"];
  tags.forEach((tag) => {
    container.querySelectorAll(tag).forEach((el) => {
      const parent = el.parentNode;
      while (el.firstChild) {
        parent.insertBefore(el.firstChild, el);
      }
      parent.removeChild(el);
    });
  });
}

function surroundSelectionWithLink(url) {
  const selection = window.getSelection();
  if (!selection || selection.rangeCount === 0) return;
  const range = selection.getRangeAt(0);
  if (range.collapsed) return;
  const anchor = document.createElement("a");
  anchor.href = url;
  anchor.rel = "noopener";
  anchor.target = "_blank";
  anchor.appendChild(range.extractContents());
  range.insertNode(anchor);
  selection.removeAllRanges();
  const newRange = document.createRange();
  newRange.selectNodeContents(anchor);
  selection.addRange(newRange);
}
