import $ from "jquery"
import _ from "lodash"
import GoldenLayout from "golden-layout"
import Tabs from "./tabs"
import { triggerClick } from "./util"
import { State, Event, EventEmitter, InteractiveMode } from './common'

/** A view contains the state of a C program.
 * One can change a view in the dropdown in the top toolbar */
export default class View {
  title: string
  dom: JQuery<HTMLElement>

  public tabs: Tabs.Tab[]
  private litmus: Tabs.Litmus
  private cat: Tabs.Cat
  private layout!: GoldenLayout;
  public state!: State;

  /** Highlight has already been performed */
  private isHighlighted: boolean

  /** Source has been modified */
  private dirty: boolean

  /** Events */
  private events: { [eventName:string]: [any, Function][]; }

  /** Event emitter, the events are handled only for the current view */
  private ee: EventEmitter

  constructor (title: string, litmus: string, catname: string, cat: string, initial_state?: State, config?: GoldenLayout.Config) {
    this.tabs = []
    this.events = {}
    this.ee = {
      on: (e: Event, l: any, f: Function) => this.on(e, l, f),
      off: (f) => this.off(f),
      once: (f => f(this.state)),
      emit: (e: Event, ...args: any[]) => this.emit (e, ...args)
    }
    this.dirty = true
    this.on('dirty', this, () => {
      if (!this.dirty) {
        delete this.state.interactive
        this.state.bmc_executions = []
        this.emit('clear')
        this.emit('updateArena')
        this.emit('updateMemory')
        this.emit('updateExecutionGraph')
        this.emit('updateUI')
        this.dirty = true
      }
    })
    this.title  = title
    this.isHighlighted = false
    if (initial_state)
      this.state = initial_state
    else
      this.setStateEmpty()

    this.litmus = new Tabs.Litmus(title, litmus, this.ee)
    this.tabs.push(this.litmus)

    this.cat = new Tabs.Cat(catname, cat, this.ee)
    this.tabs.push(this.cat)

    this.dom = $('<div class="view"></div>')
    $('#views').append(this.dom)
    this.initLayout(config)
  }

  private initLayout(config?: GoldenLayout.Config) {
    function component(title: string) {
      return {
        type: 'component',
        componentName: 'tab',
        componentState: { tab: title },
        title: title
      }
    }
    if (config == null) {
      config = {
        settings: {
          hasHeaders: true,
          constrainDragToContainer: true,
          reorderEnabled: true,
          selectionEnabled: false,
          popoutWholeStack: false,
          blockedPopoutsThrowError: true,
          closePopoutsOnUnload: true,
          showPopoutIcon: false,
          showMaximiseIcon: true,
          showCloseIcon: true
        },
        dimensions: {
          borderWidth: 5,
          minItemWidth: 150,
          headerHeight: 20,
          dragProxyWidth: 300,
          dragProxyHeight: 200
        },
        labels: {
          close: 'Close',
          maximise: 'Maximise',
          minimise: 'Minimise'
        },
        content: [{
          type: 'row',
          content: [{
            type: 'column',
            content: [{
              type: 'component',
              componentName: 'litmus',
              title: this.litmus.title,
              isClosable: false
            }, {
              type: 'stack',
              content: [
                component('Console'),
                /*component('Stdout'),
                component('Stderr'),
                component('Memory')*/
              ]
            }
            ]
          }, {
            type: 'stack',
            content: [{
              type: 'component',
              componentName: 'cat',
              title: this.cat.title,
              isClosable: false
            }
            ]
          }, {
            type: 'stack',
            content: [
              component('Memory')
            ]
          }
          ]
        }]
      }
    }
    interface ContentItem extends GoldenLayout.ContentItem {
      componentName: string
      content: Tabs.Tab
    }
    let self = this // WARN: Golden Layout does not work with arrow function
    this.layout = new GoldenLayout (config, this.dom);
    this.layout.registerComponent('litmus', function (container: GoldenLayout.Container, state: any) {
      (container.parent as ContentItem).content = self.litmus
      container.getElement().append(self.litmus.dom)
      self.litmus.refresh()
    })
    this.layout.registerComponent('cat', function (container: GoldenLayout.Container, state: any) {
      (container.parent as ContentItem).content = self.cat
      container.getElement().append(self.cat.dom)
      self.cat.refresh()
    })
    this.layout.registerComponent('tab', function (container: GoldenLayout.Container, state: any) {
      const tab = Tabs.create(state.tab, self.ee, state.args)
      self.tabs.push(tab);
      (container.parent as ContentItem).content = tab
      container.getElement().append(tab.dom)
      container.setState(state)
      tab.initial(self.state)
      tab.refresh()
      const unsafeTab: any = tab
      if (unsafeTab.highlight && self.state.options.colour_all)
        unsafeTab.highlight(self.state)
    })
    this.layout.on('itemDestroyed', (c: ContentItem) => {
      if (c.componentName == 'tab') {
        for (let i = 0; i < this.tabs.length; i++) {
          if (this.tabs[i] === c.content) {
            this.tabs.splice(i, 1)
            break
          }
        }
        this.off(c.content)
      }
    })
    this.layout.on( 'tabCreated', (header: GoldenLayout.Tab) => {
      if (header.contentItem.isComponent) {
        let tab = (header.contentItem as ContentItem).content
        header.element.on('mousedown', () => tab.refresh())
        tab.setActive = () => triggerClick(header.element[0])
        tab.close = () => header.contentItem.remove()
      }
    })
    this.layout.on('stateChanged', () => this.emit('layoutChanged'))
    this.layout.init()
  }

  private setStateEmpty() {
    this.state = {
      title: () => this.title,
      litmus: () => this.litmus.getValue(),
      cat: () => this.cat.getValue(),
      dirty: true,
      locs: [],
      console: '',
      model: {
        alloc_model: 'concrete',
        core_options: {
          rewrite: false,
          sequentialise: true
        },
        exec_options: {
          libc: false
        },
        switches: ['PNVI_ae_udi', 'strict_pointer_relationals', 'zap_dead_pointers']
      },
      interactiveMode: InteractiveMode.Memory,
      interactive: undefined,
      options: {
        show_integer_provenances: true,
        show_string_literals: false,
        show_pointer_bytes: false,
        hide_tau: true,
        colour_all: false,
        colour_cursor: true,
        show_mem_order: false,
        align_allocs: false,
      },
      bmc_model: 'bmc_c11',
      bmc_executions: []
    }
  }

  findTab(title: string) {
    for (let i = 0; i < this.tabs.length; i++) {
      if (Tabs.instanceOf(this.tabs[i], title)) {
        return this.tabs[i]
      }
    }
    return null
  }

  newTab(tab: string, title?: string, notClosable?: boolean, ...args: any []) {
    if (title === undefined) title = tab;
    this.layout.root.contentItems[0].addChild({
      type: 'component',
      componentName: 'tab',
      isClosable: !notClosable,
      title: title,
      componentState: { tab: tab, args: args }
    })
    this.refresh()
  }

  /** Restart interactive mode in all the tabs */
  resetInteractive() {
    delete this.state.interactive
    this.state.console = ''
  }

  /** Restart interactive execution */
  restartInteractive() {
    this.resetInteractive()
    this.emit('clear')
    this.emit('updateExecution')
    this.emit('updateExecutionGraph')
    this.emit('updateMemory')
    this.emit('updateUI')
  }

  setInteractiveMode(mode: InteractiveMode) {
    this.state.interactiveMode = mode
  }

  getEncodedState() {
    let miniConfig = GoldenLayout.minifyConfig(this.layout.toConfig())
    miniConfig.title = this.litmus.title
    miniConfig.source = this.litmus.getValue()
    return encodeURIComponent(JSON.stringify(miniConfig))
  }

  // Return this first instance (or create a new one)
  getTab(tab_name: string, title?: string, notClosable?: boolean) {
    let tab = this.findTab(tab_name)
    if (tab == null) {
      this.newTab(tab_name, title, notClosable)
      tab = <Tabs.Tab>this.findTab(tab_name)
    }
    return tab
  }

  getLitmus(): Readonly<Tabs.Litmus> {
    return this.litmus
  }

  getCat(): Readonly<Tabs.Cat> {
    return this.cat
  }

  getConsole() {
    return this.getTab('Console')
  }

  show() {
    this.dom.show()
  }

  hide() {
    this.dom.hide()
  }

  refresh () {
    this.tabs.map((tab) => tab.refresh())
    this.layout.updateSize()
  }

  isDirty(): Readonly<boolean> {
    return this.dirty
  }

  on(e: Event, l: any, f: Function) {
    let listeners = this.events[e]
    if (!listeners) {
      listeners = []
      this.events[e] = listeners
    }
    listeners.push([l, f])
  }

  off(e_l: any) {
    if (typeof e_l === 'string') { // If an event name
      this.events[e_l] = []
    } else { // If a listener (unsubscribe all)
      for (const e in this.events) {
        this.events[e] = this.events[e].filter(listener => listener[0] !== e_l)
      }
    }
  }

  emit(e: string, ...args: any[]) {
    switch (e) {
      case 'highlight':
        if (this.isHighlighted || !this.state.options.colour_all || this.dirty) return
        this.isHighlighted = true
        break;
      case 'clear':
        this.isHighlighted = false
        break;
      case 'mark':
        if (!this.state.options.colour_cursor || this.dirty) return
        break;
    }
    // DEBUG events
    console.log(e)
    const listeners = this.events[e]
    args.push(this.state)
    if (listeners)
      listeners.map(listener => listener[1].apply(listener[0], args))
  }

}
