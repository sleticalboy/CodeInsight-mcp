using System;

public class SharpService {
    private string name;

    public SharpService(string name) {
        this.name = name;
    }

    public int Count { get; set; }

    public string Render() {
        return name.ToString();
    }
}
