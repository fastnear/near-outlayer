// Global state for grid highlight system
export interface HighlightBox {
  x: number;
  y: number;
  width: number;
  height: number;
  element: HTMLElement;
}

let highlightBoxes: HighlightBox[] = [];
let updateCallback: (() => void) | null = null;

export function registerHighlightBox(element: HTMLElement) {
  const updateBox = () => {
    const rect = element.getBoundingClientRect();
    const existingIndex = highlightBoxes.findIndex(box => box.element === element);

    const newBox: HighlightBox = {
      x: rect.left,
      y: rect.top,
      width: rect.width,
      height: rect.height,
      element,
    };

    if (existingIndex >= 0) {
      highlightBoxes[existingIndex] = newBox;
    } else {
      highlightBoxes.push(newBox);
    }

    if (updateCallback) {
      updateCallback();
    }
  };

  // Initial update
  updateBox();

  // Update on scroll and resize
  const handleUpdate = () => updateBox();
  window.addEventListener('scroll', handleUpdate, true);
  window.addEventListener('resize', handleUpdate);

  // Cleanup function
  return () => {
    highlightBoxes = highlightBoxes.filter(box => box.element !== element);
    window.removeEventListener('scroll', handleUpdate, true);
    window.removeEventListener('resize', handleUpdate);
    if (updateCallback) {
      updateCallback();
    }
  };
}

export function getHighlightBoxes(): HighlightBox[] {
  return highlightBoxes;
}

export function setUpdateCallback(callback: (() => void) | null) {
  updateCallback = callback;
}
