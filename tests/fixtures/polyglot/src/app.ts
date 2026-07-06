export function renderDashboard(userName: string) {
  return `hello ${userName}`;
}

export class WebController {
  render() {
    return renderDashboard("demo");
  }
}
