/* Presentation only: commands, drafts and revisions remain in existing controllers. */
function createCompactControls(root=document.querySelector('main')) {
  const ns='http://www.w3.org/2000/svg';
  const paths={
    number:'M4 3h10v12H4z M7 6v6 M11 6v6 M6 8h6 M6 10h6',
    plus:'M9 4v10 M4 9h10', undo:'M6 4 2 8l4 4 M2 8h8a4 4 0 0 1 0 8',
    redo:'M12 4l4 4-4 4 M16 8h-8a4 4 0 0 0 0 8',
    moon:'M14 11A6 6 0 0 1 7 3a6 6 0 1 0 7 8',
    sun:'M9 5a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M9 1v2 M9 15v2 M1 9h2 M15 9h2 M3 3l1 1 M14 14l1 1 M3 15l1-1 M14 4l1-1',
    panel:'M3 5h10v10H3z M9 2h7v7 M16 2l-6 6',
    dock:'M3 3h12v12H3z M11 3v12 M7 6 4 9l3 3 M4 9h6',
    play:'M6 3l9 6-9 6z', stop:'M5 5h8v8H5z',
    select:'M3 2l12 8-6 1-3 5z', trash:'M3 5h12 M6 5V3h6v2 M5 5l1 10h6l1-10 M8 8v4 M10 8v4',
    move:'M9 2v14 M2 9h14 M6 5l3-3 3 3 M6 13l3 3 3-3 M5 6 2 9l3 3 M13 6l3 3-3 3',
    first:'M3 4v10 M12 4 6 9l6 5', next:'M6 4l6 5-6 5',
    refresh:'M14 5a6 6 0 1 0 1 6 M14 2v4h-4',
    group:'M2 4h10v10H2z M6 1h10v10', pencil:'M3 12l9-9 3 3-9 9-4 1z M10 5l3 3',
    membersPlus:'M2 4h9v10H2z M14 6v6 M11 9h6',
    membersMinus:'M2 4h9v10H2z M12 9h5', check:'M3 9l4 4 8-9',
    info:'M9 2a7 7 0 1 0 0 14 7 7 0 0 0 0-14 M9 8v4 M9 5v.2',
  };
  const controls={panelToggle:'panel',themeDark:'moon',themeLight:'sun',add:'number',addSum:'plus',undo:'undo',redo:'redo',run:'play',cancelRun:'stop',selectNode:'select',deleteNode:'trash',moveNode:'move',previousNodes:'first',nextNodes:'next',refreshNodes:'refresh',loadProperties:'refresh',saveProperties:'check',createGroup:'group',renameGroup:'pencil',selectGroup:'select',deleteGroup:'trash',addGroupMembers:'membersPlus',removeGroupMembers:'membersMinus',firstGroups:'first',nextGroups:'next',refreshGroups:'refresh'};
  function icon(name) {
    const svg=document.createElementNS(ns,'svg');
    for(const [key,value] of Object.entries({viewBox:'0 0 18 18',fill:'none',stroke:'currentColor','stroke-width':'1.5','stroke-linecap':'round','stroke-linejoin':'round','aria-hidden':'true',focusable:'false'}))svg.setAttribute(key,value);
    const path=document.createElementNS(ns,'path');path.setAttribute('d',paths[name]);svg.append(path);return svg;
  }
  function decorateButton(button,name) {
    const label=button.textContent.trim();if(!label)return;
    if(button.id==='panelToggle' && /dock|ドッキング|停靠/i.test(label))name='dock';
    if(button.dataset.icon===name && button.querySelector('svg') && button.getAttribute('aria-label')===label)return;
    const text=document.createElement('span');text.className='control-label';text.textContent=label;
    button.classList.add('icon-button');button.dataset.icon=name;
    button.setAttribute('aria-label',label);button.title=label;button.replaceChildren(icon(name),text);
  }
  function help(sources,heading) {
    const wrap=document.createElement('span');wrap.className='info-control';
    const button=document.createElement('button');button.type='button';button.className='info-button';button.append(icon('info'));
    const popover=document.createElement('span');popover.className='help-popover';popover.setAttribute('role','tooltip');
    popover.id=`tooltip-${sources[0].id || sources[0].parentElement.querySelector('input,select,textarea')?.id || root.querySelectorAll('.info-control').length}`;
    button.setAttribute('aria-describedby',popover.id);
    for(const source of sources){source.dataset.helpMounted='true';popover.append(source);}
    wrap.append(button,popover);heading.append(wrap);
    wrap._label=()=>heading.querySelector('label,h1,h2,h3')?.textContent || heading.textContent.replace(popover.textContent,'').trim();
    function place(){delete wrap.dataset.dismissed;const r=button.getBoundingClientRect();
      popover.style.left=`${Math.max(16,Math.min(r.right-popover.offsetWidth,innerWidth-popover.offsetWidth-16))}px`;
      popover.style.top=`${Math.max(8,Math.min(r.bottom+6,innerHeight-popover.offsetHeight-8))}px`;
    }
    button.addEventListener('pointerenter',place);button.addEventListener('focus',place);button.addEventListener('click',place);
    button.addEventListener('keydown',e=>{if(e.key==='Escape')wrap.dataset.dismissed='true';});
    wrap._place=place;
    return wrap;
  }
  function staticHelp(ids,headingId) {
    const sources=ids.map(id=>document.getElementById(id)).filter(Boolean);
    const heading=document.getElementById(headingId);if(!heading || !sources.length || sources[0].dataset.helpMounted)return;
    const row=document.createElement('div');row.className='section-heading';heading.before(row);row.append(heading);
    help(sources,row);
  }
  function update() {
    for(const [id,name] of Object.entries(controls)){const button=document.getElementById(id);if(button)decorateButton(button,name);}
    staticHelp(['description','help'],'title');staticHelp(['graphHelp'],'graphLabel');staticHelp(['groupsHelp'],'groupsLabel');
    staticHelp(['propertyNodeDescription','propertyNodeType'],'propertyNodeName');
    for(const row of root.querySelectorAll('.property-field')) {
      const sources=[...row.querySelectorAll('.property-hint:not([data-help-mounted])')];
      const heading=row.querySelector('.property-heading');
      if(sources.length && heading){const existing=heading.querySelector('.help-popover');
        if(existing){for(const source of sources){source.dataset.helpMounted='true';existing.append(source);}}
        else help(sources,heading);
      }
    }
    const info={en:'Information',ja:'説明','zh-CN':'说明'}[document.documentElement.lang] || 'Information';
    for(const wrap of root.querySelectorAll('.info-control')) { wrap.hidden=!wrap.querySelector('.help-popover').textContent.trim();wrap.querySelector('button').setAttribute('aria-label',`${info}: ${wrap._label()}`); }
    reposition();
  }
  function reposition() { for(const wrap of root.querySelectorAll('.info-control'))if(!wrap.hidden && !wrap.dataset.dismissed && wrap.matches(':hover,:focus-within'))wrap._place(); }
  window.addEventListener('resize',reposition);
  function dismiss(event) { if(event.key==='Escape')for(const wrap of root.querySelectorAll('.info-control'))wrap.dataset.dismissed='true'; }
  document.addEventListener('keydown',dismiss);
  update();const observer=new MutationObserver(update);observer.observe(root,{childList:true,subtree:true,characterData:true});
  return { refresh:update, disconnect:()=>{observer.disconnect();document.removeEventListener('keydown',dismiss);window.removeEventListener('resize',reposition);} };
}
if (typeof window !== 'undefined') window.compactControls=createCompactControls();
if (typeof module !== 'undefined') module.exports={createCompactControls};
