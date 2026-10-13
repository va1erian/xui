// The calculator's code-behind. `digit` and `operator` are control arrays:
// one handler each, given the element's index (see docs/forms.md).

fn form_load() { clear_click(); }

fn digit_click(index) {
    form.state.entry = `${form.state.entry}${index}`;
    display.text = form.state.entry;
}

fn operator_click(index) {
    equals_click();
    form.state.pending = operator[index].text;
}

fn equals_click() {
    if form.state.entry != "" {
        let value = parse_float(form.state.entry);
        let total = form.state.total;
        form.state.total = switch form.state.pending {
            "+" => total + value, "-" => total - value, "*" => total * value,
            "/" => if value == 0.0 { total } else { total / value },
            _ => value,
        };
        form.state.entry = "";
    }
    form.state.pending = "";
    display.text = `${form.state.total}`;
}

fn clear_click() {
    form.state = #{ total: 0.0, entry: "", pending: "" };
    display.text = "0";
}
