// Navigation controls (Back, Forward, Add)

export function setupNavigationControls(): void {
  const backBtn = document.getElementById('btn-nav-back');
  const fwdBtn = document.getElementById('btn-nav-forward');

  backBtn?.addEventListener('click', (e) => {
    e.stopPropagation();
    console.debug('[Navigation] Back requested');
  });

  fwdBtn?.addEventListener('click', (e) => {
    e.stopPropagation();
    console.debug('[Navigation] Forward requested');
  });
}
