<?php

use App\Repository\UserRepository;

class PhpService
{
    private string $name;

    public function __construct(string $name)
    {
        $this->name = $name;
    }

    public function render(): string
    {
        return strtoupper($this->name);
    }
}
