package fixture;

import java.util.List;

public class JavaService {
    private List<String> names;

    public JavaService(List<String> names) {
        this.names = names;
    }

    public int count() {
        return names.size();
    }
}
