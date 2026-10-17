// Asks whether to leave something unsaved behind (see `confirmLeaving`), in
// Lanwell's own style: Stay is the filled, default button, since staying is
// the safe answer; leaving is the outlined one, like Remove and Stop sharing.

import { createEffect, Show } from 'solid-js';

import { Button } from './ui';
import { question } from './unsaved';

export function LeaveDialog() {
  let dialog: HTMLDialogElement | undefined;

  // A modal <dialog>: keeps focus inside, dims the rest, and Escape closes it.
  createEffect(() => {
    if (question()) {
      dialog?.showModal();
    } else {
      dialog?.close();
    }
  });

  return (
    <dialog
      ref={dialog}
      class="backdrop:bg-black/40 m-auto max-w-sm rounded-lg border border-border bg-panel p-5 text-fg shadow-lg"
      onClose={() => question()?.answer(false)}
    >
      <Show when={question()}>
        {(asked) => (
          <>
            <h2 class="text-base font-semibold">{asked().verb} anyway?</h2>
            <p class="mt-2 text-fg-muted">{asked().what}</p>
            <div class="mt-5 flex justify-end gap-2">
              <Button variant="danger" onClick={() => asked().answer(true)}>
                {asked().verb}
              </Button>
              <Button variant="primary" autofocus onClick={() => asked().answer(false)}>
                Stay
              </Button>
            </div>
          </>
        )}
      </Show>
    </dialog>
  );
}
